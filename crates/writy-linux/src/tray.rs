use ksni::{Tray, TrayMethods};

struct Writy;

impl Tray for Writy {
    fn id(&self) -> String {
        "writy".into()
    }

    fn title(&self) -> String {
        "Writy".into()
    }

    fn icon_name(&self) -> String {
        "document-edit-symbolic".into()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: self.title(),
            description: "Writy is running".into(),
            ..Default::default()
        }
    }
}

pub async fn run() {
    if let Err(error) = Writy.assume_sni_available(true).spawn().await {
        eprintln!("Writy status icon: {error}");
    }
}
