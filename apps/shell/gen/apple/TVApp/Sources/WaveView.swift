import SwiftUI

/// «Моя волна»: одна большая кнопка (бренд «Сумерки» — единственный градиент), настроения и
/// сохранённые сочетания.
struct WaveView: View {
    @EnvironmentObject private var store: Store

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 60) {
                Button { store.startWave() } label: {
                    HStack(spacing: 40) {
                        Image(systemName: "play.fill").font(.system(size: 64))
                        VStack(alignment: .leading, spacing: 8) {
                            Text("Моя волна").font(.system(size: 64, weight: .bold))
                            Text("Знакомое и новое — под ваш вкус").font(.system(size: 30)).opacity(0.85)
                        }
                        Spacer()
                    }
                    .foregroundStyle(.white)
                    .padding(.horizontal, 70)
                    .frame(maxWidth: .infinity, minHeight: 230)
                    .background(Theme.brand)
                }
                .buttonStyle(.card)

                section("Настроение") {
                    HStack(spacing: 40) {
                        ForEach(Mood.allCases) { mood in
                            Button { store.startWave(WaveSettings(mood: mood.rawValue)) } label: {
                                VStack(spacing: 20) {
                                    Image(systemName: mood.symbol).font(.system(size: 54))
                                        .foregroundStyle(mood.color)
                                    Text(mood.title).font(.system(size: 30, weight: .semibold))
                                        .foregroundStyle(Theme.text1)
                                }
                                .frame(width: 360, height: 220)
                                .background(Theme.fill1)
                            }
                            .buttonStyle(.card)
                        }
                    }
                }

                if !store.presets.isEmpty {
                    section("Сочетания") {
                        ScrollView(.horizontal) {
                            HStack(spacing: 40) {
                                ForEach(store.presets) { preset in
                                    Button { store.startWave(preset.settings) } label: {
                                        HStack(spacing: 20) {
                                            Image(systemName: presetSymbol(preset.icon)).font(.system(size: 36))
                                                .foregroundStyle(Theme.text2)
                                            Text(preset.name).font(.system(size: 30, weight: .semibold))
                                                .foregroundStyle(Theme.text1).lineLimit(1)
                                        }
                                        .padding(.horizontal, 40)
                                        .frame(minWidth: 320, minHeight: 120)
                                        .background(Theme.fill1)
                                    }
                                    .buttonStyle(.card)
                                }
                            }
                            .padding(.vertical, 30)
                        }
                        .scrollClipDisabled()
                    }
                }
            }
            .padding(.horizontal, 90)
            .padding(.vertical, 40)
        }
        .background(Theme.ink.ignoresSafeArea())
    }

    private func section(_ title: String, @ViewBuilder content: () -> some View) -> some View {
        VStack(alignment: .leading, spacing: 26) {
            Text(title).font(.system(size: 36, weight: .bold)).foregroundStyle(Theme.text1)
            content()
        }
    }
}
