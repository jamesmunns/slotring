use core::sync::atomic::{AtomicUsize, AtomicBool, Ordering};
use core::marker::PhantomData;
use core::ptr::NonNull;

type Buffer = [u8; 8];

pub struct SlotRing<'data> {
    store: &'data mut [Buffer],
    writer: AtomicUsize,
    reader: AtomicUsize,
}

pub struct SRProd<'slotring, 'data: 'slotring> {
    _life: PhantomData<&'slotring SlotRing<'data>>,
    sr: NonNull<SlotRing<'data>>,
    wip: bool,
}

pub struct PGrant<'slotring: 'producer, 'producer, 'data: 'slotring> {
    _life: &'producer mut SRProd<'slotring, 'data>,
    buf: &'slotring mut Buffer,
}

pub struct RGrant<'slotring: 'consumer, 'consumer, 'data: 'slotring> {
    _life: &'consumer mut SRCons<'slotring, 'data>,
    buf: &'slotring Buffer,
}

pub struct SRCons<'slotring, 'data: 'slotring> {
    _life: PhantomData<&'slotring SlotRing<'data>>,
    sr: NonNull<SlotRing<'data>>,
    rip: bool,
}

impl<'data> SlotRing<'data> {
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

impl<'slotring: 'producer, 'producer, 'data: 'slotring> SRProd<'slotring, 'data> {
    pub fn start_write(&'producer mut self) -> Option<PGrant<'slotring, 'producer, 'data>> {
        let sr = unsafe { &mut *self.sr.as_ptr() };

        if self.wip {
            return None;
        }

        let idx = sr.writer.load(Ordering::SeqCst);

        // This is bad proof of concept logic
        if sr.writer.load(Ordering::SeqCst) < sr.store.len() {
            sr.writer.fetch_add(1, Ordering::SeqCst);
            return Some(PGrant {
                _life: self,
                buf: &mut sr.store[idx],
            });
        }

        None
    }
}

impl<'slotring: 'producer, 'producer, 'data: 'slotring> Drop for PGrant<'slotring, 'producer, 'data> {
    fn drop(&mut self) {
        self._life.wip = false;
    }
}

impl<'slotring: 'producer, 'producer, 'data: 'slotring> PGrant<'slotring, 'producer, 'data> {
    fn consume(self) {
    }
}

impl<'slotring: 'consumer, 'consumer, 'data: 'slotring> SRCons<'slotring, 'data> {
    pub fn start_read(&'consumer mut self) -> Option<RGrant<'slotring, 'consumer, 'data>> {
        let sr = unsafe { &mut *self.sr.as_ptr() };

        if self.rip {
            return None;
        }

        let idx = sr.reader.load(Ordering::SeqCst);

        // This is bad proof of concept logic
        if idx < sr.store.len() {
            sr.reader.fetch_add(1, Ordering::SeqCst);
            return Some(RGrant {
                _life: self,
                buf: &sr.store[idx],
            });
        }

        None
    }
}

impl<'slotring: 'consumer, 'consumer, 'data: 'slotring> Drop for RGrant<'slotring, 'consumer, 'data> {
    fn drop(&mut self) {
        self._life.rip = false;
    }
}

impl<'slotring: 'consumer, 'consumer, 'data: 'slotring> RGrant<'slotring, 'consumer, 'data> {
    fn consume(self) {
    }
}


#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn basic() {
        let mut data = [[0u8; 8]; 8];
        let mut x = SlotRing {
            store: &mut data,
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
