//! Веса → ONNX (`features [N, D] → probs [N, 3]`) без зависимостей: protobuf пишется вручную, полей
//! нужно немного. Граф — ровно [`Params::forward`]; совпадение проверяет тест сервера через onnxruntime.

use crate::{HEADS, net::Params};

/// IR 9 и opset 17 понимают onnxruntime сервера и сервиса ml.
const IR_VERSION: i64 = 9;
const OPSET: i64 = 17;
const FLOAT: i64 = 1;
const INT64: i64 = 7;

#[derive(Default)]
struct Buf(Vec<u8>);

impl Buf {
    fn varint(&mut self, mut v: u64) {
        while v >= 0x80 {
            self.0.push((v as u8) | 0x80);
            v >>= 7;
        }
        self.0.push(v as u8);
    }

    fn key(&mut self, field: u32, wire: u8) {
        self.varint((u64::from(field) << 3) | u64::from(wire));
    }

    fn int(&mut self, field: u32, v: i64) {
        self.key(field, 0);
        self.varint(v as u64);
    }

    fn bytes(&mut self, field: u32, b: &[u8]) {
        self.key(field, 2);
        self.varint(b.len() as u64);
        self.0.extend_from_slice(b);
    }

    fn str(&mut self, field: u32, s: &str) {
        self.bytes(field, s.as_bytes());
    }

    fn msg(&mut self, field: u32, m: Buf) {
        self.bytes(field, &m.0);
    }
}

/// TensorProto: dims=1, data_type=2, name=8, raw_data=9.
fn tensor_f32(name: &str, dims: &[usize], data: &[f32]) -> Buf {
    let mut t = Buf::default();
    for d in dims {
        t.int(1, *d as i64);
    }
    t.int(2, FLOAT);
    t.str(8, name);
    let raw: Vec<u8> = data.iter().flat_map(|x| x.to_le_bytes()).collect();
    t.bytes(9, &raw);
    t
}

fn tensor_i64(name: &str, data: &[i64]) -> Buf {
    let mut t = Buf::default();
    t.int(1, data.len() as i64);
    t.int(2, INT64);
    t.str(8, name);
    let raw: Vec<u8> = data.iter().flat_map(|x| x.to_le_bytes()).collect();
    t.bytes(9, &raw);
    t
}

/// NodeProto: input=1, output=2, name=3, op_type=4, attribute=5.
fn node(op: &str, inputs: &[&str], output: &str, attrs: Vec<Buf>) -> Buf {
    let mut n = Buf::default();
    for i in inputs {
        n.str(1, i);
    }
    n.str(2, output);
    n.str(3, output);
    n.str(4, op);
    for a in attrs {
        n.msg(5, a);
    }
    n
}

/// AttributeProto с целым: name=1, i=3, type=20 (INT = 2).
fn attr_int(name: &str, v: i64) -> Buf {
    let mut a = Buf::default();
    a.str(1, name);
    a.int(3, v);
    a.int(20, 2);
    a
}

/// ValueInfoProto: name=1, type=2 → TypeProto.tensor_type=1 → elem_type=1, shape=2 → dim=1
/// (dim_value=1 или dim_param=2).
fn value_info(name: &str, rows: &str, cols: usize) -> Buf {
    let mut d0 = Buf::default();
    d0.str(2, rows);
    let mut d1 = Buf::default();
    d1.int(1, cols as i64);
    let mut shape = Buf::default();
    shape.msg(1, d0);
    shape.msg(1, d1);
    let mut tt = Buf::default();
    tt.int(1, FLOAT);
    tt.msg(2, shape);
    let mut ty = Buf::default();
    ty.msg(1, tt);
    let mut v = Buf::default();
    v.str(1, name);
    v.msg(2, ty);
    v
}

/// ONNX-модель сети. Веса заранее проверены [`Params::validate`].
pub fn export(p: &Params) -> Vec<u8> {
    let (sd, md, d) = (p.sound_dim, p.semantic_dim, p.input_dim());
    let mut inits = vec![
        tensor_i64("s_start", &[0]),
        tensor_i64("s_end", &[sd as i64]),
        tensor_i64("t_start", &[sd as i64]),
        tensor_i64("t_end", &[(sd + md) as i64]),
        tensor_i64("c_start", &[(sd + md) as i64]),
        tensor_i64("c_end", &[d as i64]),
        tensor_i64("axis1", &[1]),
        tensor_f32("sound_mean", &[sd], &p.sound_mean),
        tensor_f32("sound_proj", &[sd, p.sound_k], &p.sound_proj),
        tensor_f32("scalar_mean", &[p.scalar_dim], &p.scalar_mean),
        tensor_f32("scalar_scale", &[p.scalar_dim], &p.scalar_scale),
        tensor_f32("clip_min", &[], &[-p.clip]),
        tensor_f32("clip_max", &[], &[p.clip]),
    ];
    let mut nodes = vec![
        node(
            "Slice",
            &["features", "s_start", "s_end", "axis1"],
            "snd",
            vec![],
        ),
        node("Sub", &["snd", "sound_mean"], "snd_c", vec![]),
        node("MatMul", &["snd_c", "sound_proj"], "z_s", vec![]),
        node(
            "Slice",
            &["features", "c_start", "c_end", "axis1"],
            "sc",
            vec![],
        ),
        node("Sub", &["sc", "scalar_mean"], "sc_c", vec![]),
        node("Mul", &["sc_c", "scalar_scale"], "z_c", vec![]),
    ];
    let mut parts = vec!["z_s"];
    if p.semantic_k > 0 {
        inits.push(tensor_f32("semantic_mean", &[md], &p.semantic_mean));
        inits.push(tensor_f32(
            "semantic_proj",
            &[md, p.semantic_k],
            &p.semantic_proj,
        ));
        nodes.push(node(
            "Slice",
            &["features", "t_start", "t_end", "axis1"],
            "sem",
            vec![],
        ));
        nodes.push(node("Sub", &["sem", "semantic_mean"], "sem_c", vec![]));
        nodes.push(node("MatMul", &["sem_c", "semantic_proj"], "z_t", vec![]));
        parts.push("z_t");
    }
    parts.push("z_c");
    nodes.push(node("Concat", &parts, "z", vec![attr_int("axis", 1)]));
    nodes.push(node("Clip", &["z", "clip_min", "clip_max"], "h0", vec![]));
    let mut prev = "h0".to_string();
    let last = p.layers.len() - 1;
    for (i, l) in p.layers.iter().enumerate() {
        let (w, b) = (format!("w{i}"), format!("b{i}"));
        inits.push(tensor_f32(&w, &[l.inputs, l.outputs], &l.w));
        inits.push(tensor_f32(&b, &[l.outputs], &l.b));
        let g = format!("g{i}");
        nodes.push(node("Gemm", &[&prev, &w, &b], &g, vec![]));
        if i < last {
            let h = format!("h{}", i + 1);
            nodes.push(node("Relu", &[&g], &h, vec![]));
            prev = h;
        } else {
            nodes.push(node("Sigmoid", &[&g], "probs", vec![]));
        }
    }
    // GraphProto: node=1, name=2, initializer=5, input=11, output=12
    let mut graph = Buf::default();
    for n in nodes {
        graph.msg(1, n);
    }
    graph.str(2, "dalywave_taste");
    for t in inits {
        graph.msg(5, t);
    }
    graph.msg(11, value_info("features", "N", d));
    graph.msg(12, value_info("probs", "N", HEADS.len()));
    // ModelProto: ir_version=1, producer_name=2, producer_version=3, doc_string=6, graph=7, opset_import=8
    let mut model = Buf::default();
    model.int(1, IR_VERSION);
    model.str(2, "dalywave-taste");
    model.str(3, env!("CARGO_PKG_VERSION"));
    model.str(6, "skip, complete, like");
    model.msg(7, graph);
    let mut opset = Buf::default();
    opset.str(1, "");
    opset.int(2, OPSET);
    model.msg(8, opset);
    model.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_protobuf_varints() {
        let mut b = Buf::default();
        b.varint(300);
        assert_eq!(b.0, [0xac, 0x02]);
        let mut b = Buf::default();
        b.int(1, 9);
        assert_eq!(b.0, [0x08, 0x09]);
    }

    #[test]
    fn exports_a_graph_with_all_weights() {
        let data = crate::train::tests::synthetic(300, 2);
        let (p, _) = crate::train(
            &data,
            &crate::Spec {
                max_epochs: 3,
                ..Default::default()
            },
        )
        .unwrap();
        let bytes = export(&p);
        // Веса лежат как есть: первые числа последнего слоя находятся в файле
        let tail: Vec<u8> = p.layers[0].w[..4]
            .iter()
            .flat_map(|x| x.to_le_bytes())
            .collect();
        assert!(bytes.windows(tail.len()).any(|w| w == tail.as_slice()));
        assert!(bytes.len() > p.trainable() * 4);
    }
}
