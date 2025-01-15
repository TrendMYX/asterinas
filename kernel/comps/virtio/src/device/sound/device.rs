use alloc::boxed::Box;
use alloc::sync::Arc;
use core::hint::spin_loop;
use log::debug;
use ostd::{Pod};
use ostd::early_println;
use ostd::mm::{DmaDirection, DmaStream, DmaStreamSlice, FrameAllocOptions, Infallible, VmIo, VmReader};
use ostd::sync::{LocalIrqDisabled, SpinLock, SpinLockGuard};
use ostd::trap::TrapFrame;
use crate::device::sound::config::{MessageHdr, SoundFeatures, VirtioSoundConfig, PcmFeatures, PcmFormats, PcmFrameRates};
use crate::device::VirtioDeviceError;
use crate::queue::VirtQueue;
use crate::transport::{ConfigManager, VirtioTransport};


pub type SoundCallback = dyn Fn(VmReader<Infallible>) + Send + Sync;

pub struct SoundDevice {
    config_manager: ConfigManager<VirtioSoundConfig>,
    transport: SpinLock<Box<dyn VirtioTransport>>,
    queue: SpinLock<VirtQueue>,
    txq: SpinLock<VirtQueue>,
    rxq: SpinLock<VirtQueue>,
    tx_buffer: DmaStream,
    rx_buffer: DmaStream,
    ctl_buffer: DmaStream,
    event_buffer: DmaStream,
    // callbacks: RwLock<Vec<&'static SoundCallback>, LocalIrqDisabled>,
}

impl SoundDevice {
    pub(crate) fn negotiate_features(features: u64) -> u64 {
        // let features =SoundFeatures::from_bits_truncate(features);
        // features.bits()
        features
    }

    pub fn init(mut transport: Box<dyn VirtioTransport>) -> Result<(), VirtioDeviceError> {
        let config_manager = VirtioSoundConfig::new_manager(transport.as_ref());
        let config = config_manager.read_config();
        debug!("virtio_sound_config = {:?}", config);

        debug!("begin initializing virtqueues");
        const Q_INDEX: u16 = 0;
        const TXQ_INDEX: u16 = 1;
        const RXQ_INDEX: u16 = 2;

        let message_queue = SpinLock::new(VirtQueue::new(Q_INDEX, 2, transport.as_mut()).unwrap());
        let txq = SpinLock::new(VirtQueue::new(TXQ_INDEX, 2, transport.as_mut()).unwrap());
        let rxq = SpinLock::new(VirtQueue::new(RXQ_INDEX, 2, transport.as_mut()).unwrap());


        let tx_buffer = {
            let vm_segment = FrameAllocOptions::new().alloc_segment(4).unwrap();
            DmaStream::map(vm_segment.into(), DmaDirection::ToDevice, false).unwrap()
        };

        let rx_buffer = {
            let vm_segment = FrameAllocOptions::new().alloc_segment(4).unwrap();
            DmaStream::map(vm_segment.into(), DmaDirection::FromDevice, false).unwrap()
        };

        let ctl_buffer = {
            let vm_segment = FrameAllocOptions::new().alloc_segment(100).unwrap();
            DmaStream::map(vm_segment.into(), DmaDirection::ToDevice, false).unwrap()
        };

        let event_buffer = {
            let vm_segment = FrameAllocOptions::new().alloc_segment(100).unwrap();
            DmaStream::map(vm_segment.into(), DmaDirection::FromDevice, false).unwrap()
        };

        let device = Arc::new(
            Self {
                config_manager,
                transport: SpinLock::new(transport),
                queue: message_queue,
                txq,
                rxq,
                tx_buffer,
                rx_buffer,
                ctl_buffer,
                event_buffer,
                // callbacks: RwLock::new(Vec::new()),
            });


        // Register irq callbacks
        let mut transport = device.transport.disable_irq().lock();

        fn config_space_change(_: &TrapFrame) {
            debug!("sound device config space change");
        }

        transport
            .register_cfg_callback(Box::new(config_space_change))
            .unwrap();

        transport.finish_init();
        drop(transport);


        Self::test_device(&*device);
        Ok(())
    }

    pub fn handle_event_irq(&self) {
        debug!("handling event irq");
        self.event_buffer.sync(0..PCM_INFO_SIZE).unwrap();
        let hdr = self.event_buffer.reader().unwrap().read_once::<u32>().unwrap();
        let _features = self.event_buffer.reader().unwrap().read_once::<u32>().unwrap();
        let _formats = self.event_buffer.reader().unwrap().read_once::<u64>().unwrap();
        let _rates = self.event_buffer.reader().unwrap().read_once::<u64>().unwrap();
        let _direction = self.event_buffer.reader().unwrap().read_once::<u8>().unwrap();
        let _channel_min = self.event_buffer.reader().unwrap().read_once::<u8>().unwrap();
        let _channel_max = self.event_buffer.reader().unwrap().read_once::<u8>().unwrap();
        debug!(
            "Event IRQ handled: hdr={:?}", hdr
        );
    }


    fn handle_rx_irq(&self) {
        // TODO!
    }

    fn test_device(&self) {
        // Query supported configuration
        let mut queue = self.queue.disable_irq().lock();
        early_println!("Query PCM info");
        let req = SndPcmQueryInfo {
            hdr: MessageHdr::PcmInfo as u32,
            start_id: 0,
            count: 1,
            size: PCM_INFO_QUERY_SIZE as u32,
        };
        self.send(&req, PCM_INFO_QUERY_SIZE, PCM_INFO_SIZE, &mut queue);
        self.event_buffer.sync(0..PCM_INFO_SIZE).unwrap();
        let hdr = self.event_buffer.reader().unwrap().read_once::<u32>().unwrap();
        let features = self.event_buffer.reader().unwrap().read_once::<u32>().unwrap();
        let formats = self.event_buffer.reader().unwrap().read_once::<u64>().unwrap();
        let rates = self.event_buffer.reader().unwrap().read_once::<u64>().unwrap();
        let direction = self.event_buffer.reader().unwrap().read_once::<u8>().unwrap();
        let channel_min = self.event_buffer.reader().unwrap().read_once::<u8>().unwrap();
        let channel_max = self.event_buffer.reader().unwrap().read_once::<u8>().unwrap();
        early_println!(
            "Query PCM info: hdr={:?}, features={:?}, formats={:?}, rates={:?}, direction={:?}, channel_min={:?}, channel_max={:?}",
            hdr, features, formats, rates, direction, channel_min, channel_max
        );
        // Query PCM info: hdr=32768, features=32768, formats=32768, rates=32768, direction=0, channel_min=0, channel_max=0
        
        // --------------------------------------------------------------------------------------
        //流程顺序：SetParams -> Prepared -> Start -> IO Message -> Stop >> Release

        early_println!("Set PCM params");
        let req = SndPcmSetParams {
            hdr: MessageHdr::PcmSetParams as u32,
            buffer_bytes: 1,
            period_bytes: 1,
            features: 0, 
            channels: 0,
            format: PcmFormats::FmtU8 as u8,
            rate: PcmFrameRates::Rate16000 as u8,
            padding: [0, 0, 0, 0, 0],
        };//提示:Number of channels is not supported
        // let req = SndPcmSetParams {
        //     hdr: MessageHdr::PcmSetParams as u32,
        //     buffer_bytes: 20, // ??
        //     period_bytes: 10, // 2 bytes * 5
        //     features: 1 << (PcmFeatures::MsgPolling as u32), 
        //     channels: 0,
        //     format: PcmFormats::FmtU16 as u8,
        //     rate: PcmFrameRates::Rate16000 as u8,
        //     padding: [0, 0, 0, 0, 0],
        // };对于单声道16bit采样率16000Hz的音频,但会报错Streams have not been initialized并卡死
        self.send(&req, PCM_SET_PARAMS_SIZE, 8, &mut queue);
        self.event_buffer.sync(0..8).unwrap();
        let hdr = self.event_buffer.reader().unwrap().read_once::<u32>().unwrap();
        let data = self.event_buffer.reader().unwrap().read_once::<u32>().unwrap();
        early_println!(
            "Response of setting params: hdr={:?}, data={:?}", hdr, data
        );

        // --------------------------------------------------------------------------------------
        
        early_println!("Set PCM prepared");
        let req = SndPcmHdr {
            hdr: MessageHdr::PcmPrepare as u32,
            stream_id: 0,
        };
        self.send(&req, PCM_HDR_SIZE, 8, &mut queue);
        self.event_buffer.sync(0..8).unwrap();
        let hdr = self.event_buffer.reader().unwrap().read_once::<u32>().unwrap();
        let data = self.event_buffer.reader().unwrap().read_once::<u32>().unwrap();
        early_println!(
            "Response of setting preparation: hdr={:?}, data={:?}", hdr, data
        );

        // --------------------------------------------------------------------------------------
        
        early_println!("Set PCM start");
        let req = SndPcmHdr {
            hdr: MessageHdr::PcmStart as u32,
            stream_id: 0,
        };
        self.send(&req, PCM_HDR_SIZE, 8, &mut queue);
        self.event_buffer.sync(0..8).unwrap();
        let hdr = self.event_buffer.reader().unwrap().read_once::<u32>().unwrap();
        let data = self.event_buffer.reader().unwrap().read_once::<u32>().unwrap();
        early_println!(
            "Response of setting start: hdr={:?}, data={:?}", hdr, data
        );

        // --------------------------------------------------------------------------------------
        
        // early_println!("Send PCM frames");//panic: InvalidArgs
        // let tx_slice = {
        //     let txq_slice =
        //         DmaStreamSlice::new(self.tx_buffer.clone(), 0, 5);//14);//2*5+4
        //     let req = SndPcmIOMessage {
        //         stream_id: 0 as u32,   
        //         buffer: 1,//[42, 42, 42, 42, 42],//array as [u16; 5],
        //     };
        //     txq_slice.write_val(0, &req).unwrap();
        //     txq_slice.sync().unwrap();
        //     txq_slice
        // };
        // let rx_slice = {
        //     let rx_slice =
        //         DmaStreamSlice::new(self.rx_buffer.clone(), 0, 8);
        //     rx_slice
        // };
        // let mut queue = self.queue.disable_irq().lock();
        // queue
        //     .add_dma_buf(&[&tx_slice], &[&rx_slice])
        //     .expect("add queue failed");
        // if queue.should_notify() {
        //     queue.notify();
        // }
        // while !queue.can_pop() {
        //     spin_loop();
        // }
        // queue.pop_used().unwrap();
        // self.rx_buffer.sync(0..8).unwrap();
        // let status = self.rx_buffer.reader().unwrap().read_once::<u32>().unwrap();
        // let latency_bytes = self.rx_buffer.reader().unwrap().read_once::<u32>().unwrap();
        // early_println!(
        //     "Response of IO Message: status={:?}, latency_bytes={:?}", status, latency_bytes
        // );


        // --------------------------------------------------------------------------------------


        early_println!("Stop PCM");
        let req = SndPcmHdr {
            hdr: MessageHdr::PcmStop as u32,
            stream_id: 0,
        };
        self.send(&req, PCM_HDR_SIZE, 8, &mut queue);
        self.event_buffer.sync(0..8).unwrap();
        let hdr = self.event_buffer.reader().unwrap().read_once::<u32>().unwrap();
        let data = self.event_buffer.reader().unwrap().read_once::<u32>().unwrap();
        early_println!(
            "Response of stopping PCM: hdr={:?}, data={:?}", hdr, data
        );

        // --------------------------------------------------------------------------------------

        early_println!("Release PCM");
        let req = SndPcmHdr {
            hdr: MessageHdr::PcmRelease as u32,
            stream_id: 0,
        };
        self.send(&req, PCM_HDR_SIZE, 8, &mut queue);
        self.event_buffer.sync(0..8).unwrap();
        let hdr = self.event_buffer.reader().unwrap().read_once::<u32>().unwrap();
        let data = self.event_buffer.reader().unwrap().read_once::<u32>().unwrap();
        early_println!(
            "Response of releasing PCM: hdr={:?}, data={:?}", hdr, data
        );

    }


    pub fn send<T: Pod>(&self, data: &T, send_size: usize, recv_size: usize, queue: &mut SpinLockGuard<VirtQueue, LocalIrqDisabled>) {
        let ctl_slice = {
            let req_slice =
                DmaStreamSlice::new(self.ctl_buffer.clone(), 0, send_size);
            req_slice.write_val(0, data).unwrap();
            req_slice.sync().unwrap();
            req_slice
        };

        let event_slice = {
            let resp_slice =
                DmaStreamSlice::new(self.event_buffer.clone(), 0, recv_size);
            resp_slice
        };

        queue
            .add_dma_buf(&[&ctl_slice], &[&event_slice])
            .expect("add queue failed");
        if queue.should_notify() {
            queue.notify();
        }

        while !queue.can_pop() {
            spin_loop();
        }

        queue.pop_used().unwrap();
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct SndPcmQueryInfo {
    pub hdr: u32,    // 通用信息头
    pub start_id: u32,        // 小端：起始 ID
    pub count: u32,           // 小端：查询的条目数量
    pub size: u32,            // 小端：每个条目的大小
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct SndPcmInfo {
    pub hdr: u32,   // 嵌套的通用信息头
    pub features: u32,        // 小端：特性位掩码 (1 << VIRTIO_SND_PCM_F_XXX)
    pub formats: u64,         // 小端：支持的采样格式 (1 << VIRTIO_SND_PCM_FMT_XXX)
    pub rates: u64,           // 小端：支持的采样率 (1 << VIRTIO_SND_PCM_RATE_XXX)
    pub direction: u8,        // 数据流方向 (VIRTIO_SND_D_XXX)
    pub channels_min: u8,     // 支持的最小通道数
    pub channels_max: u8,     // 支持的最大通道数
    pub padding: [u8; 5],     // 填充字节
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct SndPcmSetParams {
    pub hdr: u32,           // 头部，表示结构体的类型或标识符 (VIRTIO_SND_R_PCM_SET_PARAMS)
    pub buffer_bytes: u32,  // 缓冲区大小，单位字节
    pub period_bytes: u32,  // 每个周期的字节数
    pub features: u32,      // 特性标志位掩码 (1 << VIRTIO_SND_PCM_F_XXX)
    pub channels: u8,       // 音频通道数
    pub format: u8,         // 音频格式 (VIRTIO_SND_PCM_FMT_XXX)
    pub rate: u8,           // 采样率 (VIRTIO_SND_PCM_RATE_XXX)
    pub padding: [u8; 5],   // 填充字节，用于对齐
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct SndPcmHdr {
    pub hdr: u32, // 通用信息头
    pub stream_id: u32,    // 小端：PCM 流 ID
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct SndPcmIOMessage {
    pub stream_id: u32,   
    pub buffer: u8,//[u16; 5],
}

const PCM_HDR_SIZE: usize = size_of::<SndPcmHdr>();
const PCM_SET_PARAMS_SIZE: usize = size_of::<SndPcmSetParams>();
const PCM_INFO_QUERY_SIZE: usize = size_of::<SndPcmQueryInfo>();
const PCM_INFO_SIZE: usize = size_of::<SndPcmInfo>();
