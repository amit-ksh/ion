//! Local catalog identity marks. GPUI caches decoding; no remote URLs in frames.
use super::*;

pub(super) fn render(toolkit: &str, theme: &Theme) -> AnyElement {
    let asset = match toolkit {
        "github" => return icon("app-icons/github.svg", 26.0, theme.text).into_any_element(),
        "gmail" => "app-icons/gmail.png",
        "googledrive" => "app-icons/googledrive.png",
        "outlook" => "app-icons/outlook.svg",
        "supabase" => "app-icons/supabase.svg",
        "gitlab" => "icons/file-types/gitlab.svg",
        _ => {
            return icon(
                match toolkit {
                    "googledocs" | "googlesheets" | "notion" => "icons/file.svg",
                    _ => "icons/package.svg",
                },
                22.0,
                theme.text_secondary,
            )
            .into_any_element();
        }
    };
    img(asset)
        .size(px(26.0))
        .object_fit(ObjectFit::Contain)
        .into_any_element()
}
