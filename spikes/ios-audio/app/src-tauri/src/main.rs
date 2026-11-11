// Скрываем консоль Windows в релизной сборке.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    dw_spike_lib::run()
}
