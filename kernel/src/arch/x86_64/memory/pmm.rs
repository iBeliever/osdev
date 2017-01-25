use bitmap::{Bitmap, BITS_PER_ITEM};
use core::cmp::min;
use multiboot2::BootInformation;
use spin::Mutex;
use super::{Frame, PhysicalAddress};
use super::paging::MAX_FRAMES;

const FRAME_BITMAP_SIZE: usize = MAX_FRAMES/BITS_PER_ITEM;

pub static ALLOCATOR: Mutex<BitmapFrameAllocator> = Mutex::new(BitmapFrameAllocator::new());

pub trait FrameAllocator {
    fn allocate_frame(&mut self) -> Option<Frame>;
    fn deallocate_frame(&mut self, frame: Frame);
}

pub struct BitmapFrameAllocator {
    frame_bitmap: Bitmap<[u64; FRAME_BITMAP_SIZE]>,
    next_free_frame: usize
}

impl BitmapFrameAllocator {
    pub const fn new() -> BitmapFrameAllocator {
        BitmapFrameAllocator { frame_bitmap: Bitmap::new([u64::max_value(); FRAME_BITMAP_SIZE]),
                               next_free_frame: 0 }
    }

    pub fn mark_area_as_available(&mut self, address: PhysicalAddress, length: usize) {
        let start_frame = Frame::containing_address(address);
        let end_frame = Frame::containing_address(address + length - 1);

        if self.next_free_frame > start_frame.number {
            self.next_free_frame = start_frame.number;
        }

        for frame in Frame::range_inclusive(start_frame, end_frame) {
            self.frame_bitmap.set(frame.number, false);
        }
    }

    pub fn mark_area_in_use(&mut self, address: PhysicalAddress, length: usize) {
        let start_frame = Frame::containing_address(address);
        let end_frame = Frame::containing_address(address + length - 1);
        if self.next_free_frame >= start_frame.number && self.next_free_frame <= end_frame.number {
            self.next_free_frame = end_frame.number + 1;
        }

        for frame in Frame::range_inclusive(start_frame, end_frame) {
            self.frame_bitmap.set(frame.number, true);
        }
    }
}

impl FrameAllocator for BitmapFrameAllocator {
    fn allocate_frame(&mut self) -> Option<Frame> {
        let free_frame = {
            if self.frame_bitmap.get(self.next_free_frame) == false {
                Some(self.next_free_frame)
            } else {
                self.frame_bitmap.first_unset(self.next_free_frame)
            }
        };

        if let Some(number) = free_frame {
            self.frame_bitmap.set(number, true);
            self.next_free_frame = number + 1;

            Some(Frame { number: number })
        } else {
            None
        }
   }

   fn deallocate_frame(&mut self, frame: Frame) {
       self.frame_bitmap.set(frame.number, false);
       self.next_free_frame = min(self.next_free_frame, frame.number);
   }
}

pub fn init(boot_info: &BootInformation, kernel_end: PhysicalAddress) {
    assert_has_not_been_called!("pmm::init must be called only once");

    let mut allocator = ALLOCATOR.lock();

    let memory_map_tag = boot_info.memory_map_tag()
        .expect("Memory map tag required");

    for area in memory_map_tag.memory_areas() {
        println!("region is available: {:#x}, size: {:#x}",
                 area.base_addr, area.length);

        allocator.mark_area_as_available(area.base_addr as usize, area.length as usize);
    }

    allocator.mark_area_in_use(0x0, kernel_end);
}
