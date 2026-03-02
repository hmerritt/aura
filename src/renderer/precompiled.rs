pub static PRECOMPILED_SHADERS: &[(&str, &[u8])] =
    include!(concat!(env!("OUT_DIR"), "/precompiled_shaders.rs"));

pub fn shader_bytes(name: &str) -> Option<&'static [u8]> {
    PRECOMPILED_SHADERS
        .iter()
        .find_map(|(shader_name, bytes)| (*shader_name == name).then_some(*bytes))
}

pub fn shader_names() -> Vec<&'static str> {
    PRECOMPILED_SHADERS
        .iter()
        .map(|(shader_name, _)| *shader_name)
        .collect()
}
