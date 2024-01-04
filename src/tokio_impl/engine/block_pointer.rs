// Copyright (c) 2022-2023, Radu Racariu.

use super::single_threaded::BlockPropsType;

/// Holds a fat pointer to a BlockProps trait object.
#[derive(Default, Clone, Copy)]
pub(super) struct BlockPropsPointer {
    fat_pointer: [usize; 2],
}

impl BlockPropsPointer {
    /// Constructs the BlockProps pointer from a ref to the trait object.
    pub(super) fn new(block: &mut dyn BlockPropsType) -> Self {
        let block_props_ptr = block as *mut (dyn BlockPropsType);

        let ptr_ref = &block_props_ptr as *const *mut dyn BlockPropsType;
        let pointer_parts = ptr_ref as *const [usize; 2];

        let fat_pointer = unsafe { *pointer_parts };
        Self { fat_pointer }
    }

    /// Tries to get the pointer to the trait object from
    /// the fat pointer stored.
    /// It returns None if there is no pointer store.
    ///
    /// # Safety
    /// This would be unsafe if the pointer stored is no longer valid.
    pub(super) fn get(&self) -> Option<*mut dyn BlockPropsType> {
        if self.fat_pointer == [0; 2] {
            None
        } else {
            let ptr = {
                let pointer_parts: *const [usize; 2] = &self.fat_pointer;
                let ptr_ref = pointer_parts as *const *mut dyn BlockPropsType;
                unsafe { *ptr_ref }
            };

            Some(ptr)
        }
    }
}
