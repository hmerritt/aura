#[derive(Debug)]
pub struct ShaderAssets {
    pub name: &'static str,
    pub vertex_spirv: &'static [u8],
    pub fragment_spirv: &'static [u8],
    #[cfg(target_os = "linux")]
    pub gnome_glsl: &'static str,
    #[cfg(target_os = "linux")]
    pub plasma_vertex_qsb: &'static [u8],
    #[cfg(target_os = "linux")]
    pub plasma_fragment_qsb: &'static [u8],
}

impl ShaderAssets {
    #[cfg(not(target_os = "linux"))]
    pub const fn new(
        name: &'static str,
        vertex_spirv: &'static [u8],
        fragment_spirv: &'static [u8],
    ) -> Self {
        Self {
            name,
            vertex_spirv,
            fragment_spirv,
        }
    }

    #[cfg(target_os = "linux")]
    pub const fn new(
        name: &'static str,
        vertex_spirv: &'static [u8],
        fragment_spirv: &'static [u8],
        gnome_glsl: &'static str,
        plasma_vertex_qsb: &'static [u8],
        plasma_fragment_qsb: &'static [u8],
    ) -> Self {
        Self {
            name,
            vertex_spirv,
            fragment_spirv,
            gnome_glsl,
            plasma_vertex_qsb,
            plasma_fragment_qsb,
        }
    }
}

pub static PRECOMPILED_SHADERS: &[ShaderAssets] =
    include!(concat!(env!("OUT_DIR"), "/precompiled_shaders.rs"));

pub fn shader_assets(name: &str) -> Option<&'static ShaderAssets> {
    PRECOMPILED_SHADERS
        .iter()
        .find(|shader| shader.name == name)
}

pub fn shader_names() -> Vec<&'static str> {
    PRECOMPILED_SHADERS
        .iter()
        .map(|shader| shader.name)
        .collect()
}
