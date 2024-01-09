#[allow(dead_code)]
pub fn truncate_tail<T>(vec: Vec<T>, len: usize) -> std::vec::Vec<T> {
    // This is safe because:
    //
    // * the slice passed to `drop_in_place` is valid; the `len > self.len`
    //   case avoids creating an invalid slice, and
    // * the `len` of the vector is shrunk before calling `drop_in_place`,
    //   such that no value will be dropped twice in case `drop_in_place`
    //   were to panic once (if it panics twice, the program aborts).
    unsafe {
        // Note: It's intentional that this is `>` and not `>=`.
        //       Changing it to `>=` has negative performance
        //       implications in some cases. See #78884 for more.
        if len > vec.len() {
            return vec;
        }
        let remaining_len = vec.len() - len;
        let (ptr, _, cap) = vec.into_raw_parts();
        let new_ptr = ptr.add(remaining_len);
        let s = core::ptr::slice_from_raw_parts_mut(ptr, remaining_len);
        core::ptr::drop_in_place(s);
        Vec::from_raw_parts(new_ptr, len, cap - remaining_len)
    }
}
