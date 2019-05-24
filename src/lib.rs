use core::sync::atomic::{AtomicUsize, AtomicBool, Ordering};
use core::marker::PhantomData;
use core::ptr::NonNull;

type Buffer = [u8; 8];

pub struct SlotRing {
    store: [Buffer; 8],
    writer: AtomicUsize,
    reader: AtomicUsize,
}

pub struct SRProd<'slotring> {
    _life: PhantomData<&'slotring SlotRing>,
    sr: NonNull<SlotRing>,
    wip: bool,
}

pub struct PGrant<'slotring: 'producer, 'producer> {
    _life: &'producer mut SRProd<'slotring>,
    buf: &'slotring mut Buffer,
}

pub struct RGrant<'slotring: 'consumer, 'consumer> {
    _life: &'consumer mut SRCons<'slotring>,
    buf: &'slotring Buffer,
}

pub struct SRCons<'slotring> {
    _life: PhantomData<&'slotring SlotRing>,
    sr: NonNull<SlotRing>,
    rip: bool,
}

impl SlotRing {
    pub fn split(&mut self) -> (SRProd, SRCons) {
        (
            SRProd {
                _life: PhantomData,
                sr: unsafe { NonNull::new_unchecked(self) },
                wip: false,
            },
            SRCons {
                _life: PhantomData,
                sr: unsafe { NonNull::new_unchecked(self) },
                rip: false,
            },
        )
    }
}

impl<'slotring: 'producer, 'producer> SRProd<'slotring> {
    pub fn start_write(&'producer mut self) -> Option<PGrant<'slotring, 'producer>> {
        let sr = unsafe { &mut *self.sr.as_ptr() };

        if self.wip {
            return None;
        }

        let idx = sr.writer.load(Ordering::SeqCst);

        // This is bad proof of concept logic
        if sr.writer.load(Ordering::SeqCst) < 8 {
            sr.writer.fetch_add(1, Ordering::SeqCst);
            return Some(PGrant {
                _life: self,
                buf: &mut sr.store[idx],
            });
        }

        None
    }
}

impl<'slotring: 'producer, 'producer> Drop for PGrant<'slotring, 'producer> {
    fn drop(&mut self) {
        self._life.wip = false;
    }
}

impl<'slotring: 'producer, 'producer> PGrant<'slotring, 'producer> {
    fn consume(self) {
    }
}

impl<'slotring: 'consumer, 'consumer> SRCons<'slotring> {
    pub fn start_read(&'consumer mut self) -> Option<RGrant<'slotring, 'consumer>> {
        let sr = unsafe { &mut *self.sr.as_ptr() };

        if self.rip {
            return None;
        }

        let idx = sr.reader.load(Ordering::SeqCst);

        // This is bad proof of concept logic
        if idx < 8 {
            sr.reader.fetch_add(1, Ordering::SeqCst);
            return Some(RGrant {
                _life: self,
                buf: &sr.store[idx],
            });
        }

        None
    }
}

impl<'slotring: 'consumer, 'consumer> Drop for RGrant<'slotring, 'consumer> {
    fn drop(&mut self) {
        self._life.rip = false;
    }
}

impl<'slotring: 'consumer, 'consumer> RGrant<'slotring, 'consumer> {
    fn consume(self) {
    }
}


#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn basic() {
        let mut x = SlotRing {
            store: [[0u8; 8]; 8],
            writer: AtomicUsize::new(0),
            reader: AtomicUsize::new(0),
        };

        let (mut p, mut c) = x.split();

        for i in 0..4 {
            // Explicit drop of grant
            let a = p.start_write().unwrap();
            *a.buf = [i as u8; 8];
            a.consume();

            // Implicit drop of grant
            let b = p.start_write().unwrap();
            *b.buf = [i as u8; 8];
        }

        for i in 0..4 {
            // Explicit drop of grant
            let a = c.start_read().unwrap();
            assert!((*a.buf).len() == 8);
            (*a.buf).iter().for_each(|j| assert!(i == *j));
            a.consume();

            // Implicit drop of grant
            let b = c.start_read().unwrap();
            assert!((*b.buf).len() == 8);
            (*b.buf).iter().for_each(|j| assert!(i == *j));
        }

        println!("{:?}", x.store);
    }
}
