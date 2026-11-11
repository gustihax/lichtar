// У релізі не показувати додаткове вікно консолі на Windows
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    lichtar_lib::run()
}
