use embed_manifest::manifest::{
    ActiveCodePage, DpiAwareness, ExecutionLevel, HeapType, MaxVersionTested, Setting,
    SupportedOS,
};
use embed_manifest::{embed_manifest, new_manifest};

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        embed_manifest(
            new_manifest("AccessForge")
                .active_code_page(ActiveCodePage::Utf8)
                .dpi_awareness(DpiAwareness::PerMonitorV2)
                .requested_execution_level(ExecutionLevel::AsInvoker)
                .heap_type(HeapType::SegmentHeap)
                .long_path_aware(Setting::Enabled)
                .max_version_tested(MaxVersionTested::Windows11Version22H2)
                .supported_os(SupportedOS::Windows7..=SupportedOS::Windows81),
        )
        .expect("failed to embed manifest");
    }
}
