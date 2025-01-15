use aster_util::safe_ptr::SafePtr;
use ostd::Pod;
use crate::transport::{ConfigManager, VirtioTransport};
use core::mem::offset_of;
use int_to_c_enum::TryFromInt;

bitflags::bitflags! {
    pub struct SoundFeatures: u64{
        const VIRTIO_SND_F_CTLS = 1 << 0;
    }
}



#[derive(Debug, Pod, Clone, Copy)]
#[repr(C)]

pub struct VirtioSoundConfig {
    pub jacks: u32,      // 可用的音频接口数（输入/输出插孔数量）
    pub streams: u32,    // 可用的 PCM 流数量
    pub chmaps: u32,     // 可用的通道映射数量
    pub controls: u32,   // 可用的控制元素数量（如果支持 VIRTIO_SND_F_CTLS 功能）
}

impl VirtioSoundConfig {
    /// 创建一个配置管理器，用于读取和管理 VirtIO 声卡的配置。
    pub fn new_manager(transport: &dyn VirtioTransport) -> ConfigManager<Self> {
        // 获取设备配置空间的内存地址并安全包装
        let safe_ptr = transport
            .device_config_mem()
            .map(|mem| SafePtr::new(mem, 0));

        // 获取设备配置的 BAR 空间信息
        let bar_space = transport.device_config_bar();

        // 创建并返回配置管理器
        ConfigManager::new(safe_ptr, bar_space)
    }
}

impl ConfigManager<VirtioSoundConfig> {
    pub(super) fn read_config(&self) -> VirtioSoundConfig {
        let mut sound_config = VirtioSoundConfig::new_uninit();

        // 读取每个字段的值
        sound_config.jacks = self
            .read_once::<u32>(offset_of!(VirtioSoundConfig, jacks))
            .unwrap();
        sound_config.streams = self
            .read_once::<u32>(offset_of!(VirtioSoundConfig, streams))
            .unwrap();
        sound_config.chmaps = self
            .read_once::<u32>(offset_of!(VirtioSoundConfig, chmaps))
            .unwrap();
        sound_config.controls = self
            .read_once::<u32>(offset_of!(VirtioSoundConfig, controls))
            .unwrap();

        sound_config
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageHdr {
    // Jack control request types
    JackInfo = 1,
    JackRemap,

    // PCM control request types
    PcmInfo = 0x0100,
    PcmSetParams,
    PcmPrepare,
    PcmRelease,
    PcmStart,
    PcmStop,

    // Channel map control request types
    ChmapInfo = 0x0200,

    // Control element request types
    CtlInfo = 0x0300,
    CtlEnumItems,
    CtlRead,
    CtlWrite,
    CtlTlvRead,
    CtlTlvWrite,
    CtlTlvCommand,

    // Jack event types
    JackConnected = 0x1000,
    JackDisconnected,

    // PCM event types
    PcmPeriodElapsed = 0x1100,
    PcmXrun,

    // Control element event types
    CtlNotify = 0x1200,

    // Common status codes
    Ok = 0x8000,
    BadMsg,
    NotSupp,
    IoErr,
}




#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataDirections {
    Output = 0,
    Input,
}//the device uses one of the following data flow directions

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcmFeatures {
    ShmemHost = 0, //支持与来宾共享主机内存。
    ShmemGuest, //支持与主机共享客户内存。
    MsgPolling, //支持基于消息的传输的轮询模式，
    EvtShmemPeriods, //支持经过的周期通知共享内存传输，
    EvtXruns, //支持运行不足/超时通知。
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcmFormats {
    /* analog formats (width / physical width) */ 
    FmtImaAdpcm = 0,   /*  4 /  4 bits */ 
    FmtMuLaw,          /*  8 /  8 bits */ 
    FmtALaw,           /*  8 /  8 bits */ 
    FmtS8,             /*  8 /  8 bits */ 
    FmtU8,             /*  8 /  8 bits */ 
    FmtS16,            /* 16 / 16 bits */ 
    FmtU16,            /* 16 / 16 bits */ 
    FmtS18_3,          /* 18 / 24 bits */ 
    FmtU18_3,          /* 18 / 24 bits */ 
    FmtS20_3,          /* 20 / 24 bits */ 
    FmtU20_3,          /* 20 / 24 bits */ 
    FmtS24_3,          /* 24 / 24 bits */ 
    FmtU24_3,          /* 24 / 24 bits */ 
    FmtS20,            /* 20 / 32 bits */ 
    FmtU20,            /* 20 / 32 bits */ 
    FmtS24,            /* 24 / 32 bits */ 
    FmtU24,            /* 24 / 32 bits */ 
    FmtS32,            /* 32 / 32 bits */ 
    FmtU32,            /* 32 / 32 bits */ 
    FmtFloat,          /* 32 / 32 bits */ 
    FmtFloat64,        /* 64 / 64 bits */ 
    /* digital formats (width / physical width) */ 
    FmtDsdU8,          /*  8 /  8 bits */ 
    FmtDsdU16,         /* 16 / 16 bits */ 
    FmtDsdU32,         /* 32 / 32 bits */ 
    FmtIec958Subframe  /* 32 / 32 bits */ 
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcmFrameRates {
    Rate5512 = 0, 
    Rate8000, 
    Rate11025, 
    Rate16000, 
    Rate22050, 
    Rate32000, 
    Rate44100, 
    Rate48000, 
    Rate64000, 
    Rate88200, 
    Rate96000, 
    Rate176400, 
    Rate192000, 
    Rate384000 
}