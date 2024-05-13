use wasm_bindgen_futures::{js_sys::Promise, JsFuture};

pub trait ToJsFuture {
    fn to_future(self) -> JsFuture;
}
impl ToJsFuture for Promise {
    fn to_future(self) -> JsFuture {
        JsFuture::from(self)
    }
}
