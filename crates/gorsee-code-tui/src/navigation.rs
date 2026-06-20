#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum FocusPane {
    Menu,
    #[default]
    Files,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuPanel {
    Project,
    Timeline,
    Diff,
    Sessions,
    Models,
    Instructions,
    Skills,
    Mcp,
    Limits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuItem {
    pub label: &'static str,
    pub icon: &'static str,
    pub panel: MenuPanel,
}

pub const MENU_ITEMS: &[MenuItem] = &[
    MenuItem {
        label: "Проект",
        icon: "",
        panel: MenuPanel::Project,
    },
    MenuItem {
        label: "Лента",
        icon: "",
        panel: MenuPanel::Timeline,
    },
    MenuItem {
        label: "Дифф",
        icon: "",
        panel: MenuPanel::Diff,
    },
    MenuItem {
        label: "Сессии",
        icon: "",
        panel: MenuPanel::Sessions,
    },
    MenuItem {
        label: "Модели",
        icon: "",
        panel: MenuPanel::Models,
    },
    MenuItem {
        label: "Инструкции",
        icon: "",
        panel: MenuPanel::Instructions,
    },
    MenuItem {
        label: "Скиллы",
        icon: "",
        panel: MenuPanel::Skills,
    },
    MenuItem {
        label: "MCP",
        icon: "",
        panel: MenuPanel::Mcp,
    },
    MenuItem {
        label: "Лимиты",
        icon: "",
        panel: MenuPanel::Limits,
    },
];
