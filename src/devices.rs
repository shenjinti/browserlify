use chromiumoxide::handler::viewport;
use lazy_static::lazy_static;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct ScreenSize {
    width: u32,
    height: u32,
}
#[derive(Clone, Debug, Serialize)]
pub struct DeviceScreen {
    device_pixel_ratio: f64,
    horizontal: ScreenSize,
    vertical: ScreenSize,
}
#[derive(Clone, Debug, Serialize)]
pub struct Device {
    title: String,
    has_touch: bool,
    emulating_mobile: bool,
    user_agent: String,
    accept_language: String,
    screen: DeviceScreen,
}

impl Device {
    pub fn new(
        title: String,
        has_touch: bool,
        emulating_mobile: bool,
        user_agent: String,
        accept_language: String,
        screen: DeviceScreen,
    ) -> Self {
        Self {
            title,
            has_touch,
            emulating_mobile,
            user_agent,
            accept_language,
            screen,
        }
    }
}

impl Device {
    pub fn get_viewport(&self, landscape: bool) -> viewport::Viewport {
        let (width, height) = if landscape {
            (self.screen.horizontal.width, self.screen.horizontal.height)
        } else {
            (self.screen.vertical.width, self.screen.vertical.height)
        };
        viewport::Viewport {
            width,
            height,
            device_scale_factor: Some(self.screen.device_pixel_ratio),
            emulating_mobile: self.emulating_mobile,
            has_touch: self.has_touch,
            is_landscape: landscape,
        }
    }
}

lazy_static! {
    static ref LAPTOP_WITH_TOUCH: Device = Device {
        title: "Laptop with touch".to_string(),
        user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 11_0_0) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/87.0.4280.88 Safari/537.36"
            .to_string(),
        has_touch: true,
        emulating_mobile: false,
        accept_language: "en".to_string(),
        screen: DeviceScreen {
            device_pixel_ratio: 1.0,
            horizontal: ScreenSize {
                width: 1280,
                height: 950,
            },
            vertical: ScreenSize {
                width: 950,
                height: 1280,
            },
        },
    };
    static ref LAPTOP_WITH_HIDPI_SCREEN: Device = Device {
        title: "Laptop with HiDPI screen".to_string(),
        user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 11_0_0) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/87.0.4280.88 Safari/537.36"
            .to_string(),
        has_touch: false,
        emulating_mobile: false,
        accept_language: "en".to_string(),
        screen: DeviceScreen {
            device_pixel_ratio: 2.0,
            horizontal: ScreenSize {
                width: 1440,
                height: 900,
            },
            vertical: ScreenSize {
                width: 900,
                height: 1440,
            },
        },
    };

    static ref LAPTOP_WITH_MDPI_SCREEN: Device = Device {
        title: "Laptop with MDPI screen".to_string(),
        user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 11_0_0) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/87.0.4280.88 Safari/537.36"
            .to_string(),
        has_touch: false,
        emulating_mobile: false,
        accept_language: "en".to_string(),
        screen: DeviceScreen {
            device_pixel_ratio: 1.0,
            horizontal: ScreenSize {
                width: 1280,
                height: 800,
            },
            vertical: ScreenSize {
                width: 800,
                height: 1280,
            },
        },
    };
    static ref WIDE_HIDPI_SCREEN: Device = Device {
        title: "HiDPI Widescreen 2560 x 1440".to_string(),
        user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 11_0_0) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/87.0.4280.88 Safari/537.36"
            .to_string(),
        has_touch: false,
        emulating_mobile: false,
        accept_language: "en".to_string(),
        screen: DeviceScreen {
            device_pixel_ratio: 2.0,
            horizontal: ScreenSize {
                width: 2560,
                height: 1440,
            },
            vertical: ScreenSize {
                width: 1440,
                height: 2560,
            },
        },
    };

    static ref LAPTOP_1920_SCREEN: Device = Device {
        title: "HiDPI Laptop 1920 x 1080".to_string(),
        user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 11_0_0) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/87.0.4280.88 Safari/537.36"
            .to_string(),
        has_touch: false,
        emulating_mobile: false,
        accept_language: "en".to_string(),
        screen: DeviceScreen {
            device_pixel_ratio: 2.0,
            horizontal: ScreenSize {
                width: 1920,
                height: 1080,
            },
            vertical: ScreenSize {
                width: 1080,
                height: 1920,
            },
        },
    };

    static ref LAPTOP_4K_SCREEN: Device = Device {
        title: "4K Laptop 1920 x 1080".to_string(),
        user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 11_0_0) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/87.0.4280.88 Safari/537.36"
            .to_string(),
        has_touch: false,
        emulating_mobile: false,
        accept_language: "en".to_string(),
        screen: DeviceScreen {
            device_pixel_ratio: 3.0,
            horizontal: ScreenSize {
                width: 1920,
                height: 1080,
            },
            vertical: ScreenSize {
                width: 1080,
                height: 1920,
            },
        },
    };

    static ref IPHONE_6_7_8: Device = Device {
        title: "iPhone 6/7/8".to_string(),
        user_agent: "Mozilla/5.0 (iPhone; CPU iPhone OS 13_2_3 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/13.0.3 Mobile/15E148 Safari/604.1"
            .to_string(),
        has_touch: true,
        emulating_mobile: true,
        accept_language: "en".to_string(),
        screen: DeviceScreen {
            device_pixel_ratio: 2.0,
            horizontal: ScreenSize {
                width: 667,
                height: 375,
            },
            vertical: ScreenSize {
                width: 375,
                height: 667,
            },
        },
    };

    static ref IPHONE_6_7_8_PLUS: Device = Device {
        title: "iPhone 6/7/8 Plus".to_string(),
        user_agent: "Mozilla/5.0 (iPhone; CPU iPhone OS 13_2_3 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/13.0.3 Mobile/15E148 Safari/604.1"
            .to_string(),
        has_touch: true,
        emulating_mobile: true,
        accept_language: "en".to_string(),
        screen: DeviceScreen {
            device_pixel_ratio: 3.0,
            horizontal: ScreenSize {
                width: 736,
                height: 414,
            },
            vertical: ScreenSize {
                width: 414,
                height: 736,
            },
        },
    };

    static ref IPHONE_X: Device = Device {
        title: "iPhone X".to_string(),
        user_agent: "Mozilla/5.0 (iPhone; CPU iPhone OS 13_2_3 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/13.0.3 Mobile/15E148 Safari/604.1"
            .to_string(),
        has_touch: true,
        emulating_mobile: true,
        accept_language: "en".to_string(),
        screen: DeviceScreen {
            device_pixel_ratio: 3.0,
            horizontal: ScreenSize {
                width: 812,
                height: 375,
            },
            vertical: ScreenSize {
                width: 375,
                height: 812,
            },
        },
    };

    static ref IPHONE_13: Device = Device {
        title: "iPhone 13".to_string(),
        user_agent: "Mozilla/5.0 (iPhone; CPU iPhone OS 15_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/15.0 Mobile/15E148 Safari/604.1"
            .to_string(),
        has_touch: true,
        emulating_mobile: true,
        accept_language: "en".to_string(),
        screen: DeviceScreen {
            device_pixel_ratio: 3.0,
            horizontal: ScreenSize {
                width: 844,
                height: 390,
            },
            vertical: ScreenSize {
                width: 390,
                height: 844,
            },
        },
    };

    static ref IPAD_MINI: Device = Device {
        title: "iPad Mini".to_string(),
        user_agent: "Mozilla/5.0 (iPad; CPU OS 11_0 like Mac OS X) AppleWebKit/604.1.34 (KHTML, like Gecko) Version/11.0 Mobile/15A5341f Safari/604.1"
            .to_string(),
        has_touch: true,
        emulating_mobile: true,
        accept_language: "en".to_string(),
        screen: DeviceScreen {
            device_pixel_ratio: 2.0,
            horizontal: ScreenSize {
                width: 1024,
                height: 768,
            },
            vertical: ScreenSize {
                width: 768,
                height: 1024,
            },
        },
    };

    static ref IPAD: Device = Device {
        title: "iPad".to_string(),
        user_agent: "Mozilla/5.0 (iPad; CPU OS 11_0 like Mac OS X) AppleWebKit/604.1.34 (KHTML, like Gecko) Version/11.0 Mobile/15A5341f Safari/604.1"
            .to_string(),
        has_touch: true,
        emulating_mobile: true,
        accept_language: "en".to_string(),
        screen: DeviceScreen {
            device_pixel_ratio: 2.0,
            horizontal: ScreenSize {
                width: 1024,
                height: 768,
            },
            vertical: ScreenSize {
                width: 768,
                height: 1024,
            },
        },
    };

    static ref IPAD_PRO: Device = Device {
        title: "iPad Pro".to_string(),
        user_agent: "Mozilla/5.0 (iPad; CPU OS 11_0 like Mac OS X) AppleWebKit/604.1.34 (KHTML, like Gecko) Version/11.0 Mobile/15A5341f Safari/604.1"
            .to_string(),
        has_touch: true,
        emulating_mobile: true,
        accept_language: "en".to_string(),
        screen: DeviceScreen {
            device_pixel_ratio: 2.0,
            horizontal: ScreenSize {
                width: 1366,
                height: 1024,
            },
            vertical: ScreenSize {
                width: 1024,
                height: 1366,
            },
        },
    };

    static ref NEXUS_10: Device = Device {
        title: "Nexus 10".to_string(),
        user_agent: "Mozilla/5.0 (Linux; Android 6.0.1; Nexus 10 Build/MOB31T) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/%s Safari/537.36"
            .to_string(),
        has_touch: true,
        emulating_mobile: true,
        accept_language: "en".to_string(),
        screen: DeviceScreen {
            device_pixel_ratio: 2.0,
            horizontal: ScreenSize {
                width: 1280,
                height: 800,
            },
            vertical: ScreenSize {
                width: 800,
                height: 1280,
            },
        },
    };

    static ref SURFACE_DUO: Device = Device {
        title: "Surface Duo".to_string(),
        user_agent: "Mozilla/5.0 (Linux; Android 8.0; Pixel 2 Build/OPD3.170816.012) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/%s Mobile Safari/537.36"
            .to_string(),
        has_touch: true,
        emulating_mobile: true,
        accept_language: "en".to_string(),
        screen: DeviceScreen {
            device_pixel_ratio: 2.0,
            horizontal: ScreenSize {
                width: 720,
                height: 540,
            },
            vertical: ScreenSize {
                width: 540,
                height: 720,
            },
        },
    };

    static ref GALAXY_NOTE_3: Device = Device {
        title: "Galaxy Note 3".to_string(),
        user_agent: "Mozilla/5.0 (Linux; U; Android 4.3; en-us; SM-N900T Build/JSS15J) AppleWebKit/534.30 (KHTML, like Gecko) Version/4.0 Mobile Safari/534.30"
            .to_string(),
        has_touch: true,
        emulating_mobile: true,
        accept_language: "en".to_string(),
        screen: DeviceScreen {
            device_pixel_ratio: 3.0,
            horizontal: ScreenSize {
                width: 640,
                height: 360,
            },
            vertical: ScreenSize {
                width: 360,
                height: 640,
            },
        },
    };
}

pub fn get_device(name: &str) -> Option<Device> {
    match name {
        "laptop-touch" => Some(LAPTOP_WITH_TOUCH.clone()),
        "laptop-hidpi" => Some(LAPTOP_WITH_HIDPI_SCREEN.clone()),
        "laptop-mdpi" => Some(LAPTOP_WITH_MDPI_SCREEN.clone()),
        "wide-hidpi" => Some(WIDE_HIDPI_SCREEN.clone()),
        "laptop-1920" => Some(LAPTOP_1920_SCREEN.clone()),
        "4k" => Some(LAPTOP_4K_SCREEN.clone()),
        "iphone-6-7-8" => Some(IPHONE_6_7_8.clone()),
        "iphone-6-7-8-plus" => Some(IPHONE_6_7_8_PLUS.clone()),
        "iphone-x" => Some(IPHONE_X.clone()),
        "iphone-13" => Some(IPHONE_13.clone()),
        "ipad-mini" => Some(IPAD_MINI.clone()),
        "ipad" => Some(IPAD.clone()),
        "ipad-pro" => Some(IPAD_PRO.clone()),
        "nexus-10" => Some(NEXUS_10.clone()),
        "surface-duo" => Some(SURFACE_DUO.clone()),
        "galaxy-note-3" => Some(GALAXY_NOTE_3.clone()),
        _ => None,
    }
}
