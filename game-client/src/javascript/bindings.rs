use js_sys::Function;
use wasm_bindgen::prelude::wasm_bindgen;


#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    pub fn log(s: &str);
    pub fn alert(s: &str);
    pub fn listenToSocketData(f: &Function);
    pub fn sendDataToSocket(data: Vec<u8>);
}

#[macro_export]
macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}