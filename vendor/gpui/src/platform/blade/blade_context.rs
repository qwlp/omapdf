use super::BladePipelines;
use anyhow::Context as _;
use blade_graphics as gpu;
use collections::HashMap;
use parking_lot::Mutex;
use std::sync::Arc;
use util::ResultExt;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct PipelineCacheKey {
    pub format: gpu::TextureFormat,
    pub alpha: gpu::AlphaMode,
    pub path_sample_count: u32,
}

#[derive(Clone)]
pub struct BladeContext {
    pub(super) gpu: Arc<gpu::Context>,
    pub(super) pipelines: Arc<Mutex<HashMap<PipelineCacheKey, Arc<BladePipelines>>>>,
}

impl BladeContext {
    pub fn new() -> anyhow::Result<Self> {
        let device_id_forced = match std::env::var("ZED_DEVICE_ID") {
            Ok(val) => parse_pci_id(&val)
                .context("Failed to parse device ID from `ZED_DEVICE_ID` environment variable")
                .log_err(),
            Err(std::env::VarError::NotPresent) => None,
            err => {
                err.context("Failed to read value of `ZED_DEVICE_ID` environment variable")
                    .log_err();
                None
            }
        };
        let gpu = Arc::new(
            unsafe {
                gpu::Context::init(gpu::ContextDesc {
                    presentation: true,
                    validation: false,
                    device_id: device_id_forced.unwrap_or(0),
                    ..Default::default()
                })
            }
            .map_err(|e| anyhow::anyhow!("{e:?}"))?,
        );
        Ok(Self {
            gpu,
            pipelines: Arc::new(Mutex::new(HashMap::default())),
        })
    }

    pub(super) fn get_or_create_pipelines(
        &self,
        surface_info: gpu::SurfaceInfo,
        path_sample_count: u32,
    ) -> Arc<BladePipelines> {
        let key = PipelineCacheKey {
            format: surface_info.format,
            alpha: surface_info.alpha,
            path_sample_count,
        };
        if let Some(existing) = self.pipelines.lock().get(&key).cloned() {
            return existing;
        }
        let created = Arc::new(BladePipelines::new(
            Arc::clone(&self.gpu),
            surface_info,
            path_sample_count,
        ));
        let mut cache = self.pipelines.lock();
        Arc::clone(cache.entry(key).or_insert(created))
    }
}

fn parse_pci_id(id: &str) -> anyhow::Result<u32> {
    let mut id = id.trim();

    if id.starts_with("0x") || id.starts_with("0X") {
        id = &id[2..];
    }
    let is_hex_string = id.chars().all(|c| c.is_ascii_hexdigit());
    let is_4_chars = id.len() == 4;
    anyhow::ensure!(
        is_4_chars && is_hex_string,
        "Expected a 4 digit PCI ID in hexadecimal format"
    );

    u32::from_str_radix(id, 16).context("parsing PCI ID as hex")
}

#[cfg(test)]
mod tests {
    use super::parse_pci_id;

    #[test]
    fn test_parse_device_id() {
        assert!(parse_pci_id("0xABCD").is_ok());
        assert!(parse_pci_id("ABCD").is_ok());
        assert!(parse_pci_id("abcd").is_ok());
        assert!(parse_pci_id("1234").is_ok());
        assert!(parse_pci_id("123").is_err());
        assert_eq!(
            parse_pci_id(&format!("{:x}", 0x1234)).unwrap(),
            parse_pci_id(&format!("{:X}", 0x1234)).unwrap(),
        );

        assert_eq!(
            parse_pci_id(&format!("{:#x}", 0x1234)).unwrap(),
            parse_pci_id(&format!("{:#X}", 0x1234)).unwrap(),
        );
        assert_eq!(
            parse_pci_id(&format!("{:#x}", 0x1234)).unwrap(),
            parse_pci_id(&format!("{:#X}", 0x1234)).unwrap(),
        );
    }
}
