pub struct NavItem {
    pub key: &'static str,
    pub path: &'static str,
    pub label: &'static str,
}

pub const NAV_ITEMS: &[NavItem] = &[
    NavItem {
        key: "todos",
        path: "/",
        label: "Todos",
    },
    NavItem {
        key: "labels",
        path: "/labels",
        label: "Labels",
    },
    NavItem {
        key: "history",
        path: "/history",
        label: "History",
    },
];
