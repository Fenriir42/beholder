use beholder_ipc::IpcError;
use serde::Serialize;
use serde::de::DeserializeOwned;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = invoke, catch)]
    async fn tauri_invoke(cmd: &str, args: JsValue) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = invoke, catch)]
    async fn tauri_invoke_raw(
        cmd: &str,
        body: JsValue,
        options: JsValue,
    ) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "event"], js_name = listen, catch)]
    async fn tauri_listen(event: &str, handler: &js_sys::Function) -> Result<JsValue, JsValue>;
}

fn to_js<T: Serialize>(v: &T) -> Result<JsValue, IpcError> {
    v.serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(|_| IpcError::Server)
}

fn from_js<T: DeserializeOwned>(v: JsValue) -> Result<T, IpcError> {
    serde_wasm_bindgen::from_value(v).map_err(|_| IpcError::Server)
}

fn error(e: JsValue) -> IpcError {
    from_js(e).unwrap_or(IpcError::Server)
}

pub async fn call<R: DeserializeOwned>(cmd: &str, args: impl Serialize) -> Result<R, IpcError> {
    let out = tauri_invoke(cmd, to_js(&args)?).await.map_err(error)?;
    from_js(out)
}

pub async fn call0<R: DeserializeOwned>(cmd: &str) -> Result<R, IpcError> {
    let out = tauri_invoke(cmd, JsValue::UNDEFINED).await.map_err(error)?;
    from_js(out)
}

async fn upload<R: DeserializeOwned>(
    cmd: &str,
    headers: &[(&str, &str)],
    file: web_sys::File,
) -> Result<R, IpcError> {
    let buf = JsFuture::from(file.array_buffer())
        .await
        .map_err(|_| IpcError::Storage)?;
    let body = js_sys::Uint8Array::new(&buf);
    let h = js_sys::Object::new();
    for (k, v) in headers {
        js_sys::Reflect::set(&h, &(*k).into(), &(*v).into()).map_err(|_| IpcError::Storage)?;
    }
    let options = js_sys::Object::new();
    js_sys::Reflect::set(&options, &"headers".into(), &h).map_err(|_| IpcError::Storage)?;
    let out = tauri_invoke_raw(cmd, body.into(), options.into())
        .await
        .map_err(error)?;
    from_js(out)
}

pub async fn upload_cover(campaign_id: &str, file: web_sys::File) -> Result<(), IpcError> {
    upload("upload_cover", &[("campaign-id", campaign_id)], file).await
}

pub async fn upload_image(file: web_sys::File) -> Result<String, IpcError> {
    upload("upload_image", &[], file).await
}

pub fn listen<T: DeserializeOwned + 'static>(event: &'static str, mut f: impl FnMut(T) + 'static) {
    let handler = Closure::<dyn FnMut(JsValue)>::new(move |e: JsValue| {
        let payload = js_sys::Reflect::get(&e, &"payload".into()).unwrap_or(JsValue::NULL);
        if let Ok(p) = from_js(payload) {
            f(p);
        }
    });
    leptos::task::spawn_local(async move {
        let _ = tauri_listen(event, handler.as_ref().unchecked_ref()).await;
        handler.forget();
    });
}
