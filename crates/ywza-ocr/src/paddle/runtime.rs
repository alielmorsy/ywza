use anyhow::Result;
use ort::environment::EnvironmentBuilder;

/// With `ocr-paddle-dynamic` the library is resolved here, so a missing or unusable
/// onnxruntime is an error instead of the panic ort raises on first use: `ORT_DYLIB_PATH`,
/// else next to the executable, else the system search path.
#[cfg(feature = "ocr-paddle-dynamic")]
pub fn environment() -> Result<EnvironmentBuilder> {
    use anyhow::anyhow;
    let name = format!(
        "{}onnxruntime{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    );
    let path = match std::env::var_os("ORT_DYLIB_PATH").filter(|p| !p.is_empty()) {
        Some(p) => p.into(),
        None => std::env::current_exe()
            .ok()
            .and_then(|e| Some(e.parent()?.join(&name)))
            .filter(|p| p.exists())
            .unwrap_or_else(|| name.clone().into()),
    };
    ort::environment::init_from(&path).map_err(|e| {
        anyhow!(
            "cannot load the ONNX Runtime library `{}`: {e}. Put {name} next to the executable \
             or set ORT_DYLIB_PATH to its full path",
            path.display()
        )
    })
}

#[cfg(not(feature = "ocr-paddle-dynamic"))]
pub fn environment() -> Result<EnvironmentBuilder> {
    Ok(ort::environment::init())
}
