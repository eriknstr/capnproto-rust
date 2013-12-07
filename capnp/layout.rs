/*
 * Copyright (c) 2013, David Renshaw (dwrenshaw@gmail.com)
 *
 * See the LICENSE file in the capnproto-rust root directory.
 */

use common::*;
use endian::*;
use mask::*;
use arena::*;
use blob::*;
use std;

#[repr(u8)]
#[deriving(Eq)]
pub enum FieldSize {
    VOID = 0,
    BIT = 1,
    BYTE = 2,
    TWO_BYTES = 3,
    FOUR_BYTES = 4,
    EIGHT_BYTES = 5,
    POINTER = 6,
    INLINE_COMPOSITE = 7
}

pub fn data_bits_per_element(size : FieldSize) -> BitCount0 {
    match size {
        VOID => 0,
        BIT => 1,
        BYTE => 8,
        TWO_BYTES => 16,
        FOUR_BYTES => 32,
        EIGHT_BYTES => 64,
        POINTER => 0,
        INLINE_COMPOSITE => 0
    }
}

pub fn pointers_per_element(size : FieldSize) -> WirePointerCount {
    match size {
        POINTER => 1,
        _ => 0
    }
}

pub enum Kind {
  PRIMITIVE,
  BLOB,
  ENUM,
  STRUCT,
  UNION,
  INTERFACE,
  LIST,
  UNKNOWN
}

pub struct StructSize {
    data : WordCount16,
    pointers : WirePointerCount16,
    preferred_list_encoding : FieldSize
}

impl StructSize {
    pub fn total(&self) -> WordCount {
        (self.data as WordCount) + (self.pointers as WordCount) * WORDS_PER_POINTER
    }
}

#[repr(u8)]
#[deriving(Eq)]
pub enum WirePointerKind {
    WP_STRUCT = 0,
    WP_LIST = 1,
    WP_FAR = 2,
    WP_CAPABILITY = 3
}


pub struct WirePointer {
    offset_and_kind : WireValue<u32>,
    upper32bits : u32,
}

pub struct StructRef {
    data_size : WireValue<WordCount16>,
    ptr_count : WireValue<WirePointerCount16>
}

pub struct ListRef {
    element_size_and_count : WireValue<u32>
}

pub struct FarRef {
    segment_id : WireValue<u32>
}

impl StructRef {
    pub fn word_size(&self) -> WordCount {
        self.data_size.get() as WordCount +
            self.ptr_count.get() as WordCount * WORDS_PER_POINTER
    }

    #[inline]
    pub fn set(&mut self, size : StructSize) {
        self.data_size.set(size.data);
        self.ptr_count.set(size.pointers);
    }
}

impl ListRef {
    #[inline]
    pub fn element_size(&self) -> FieldSize {
        unsafe {
            std::cast::transmute( (self.element_size_and_count.get() & 7) as u8)
        }
    }

    #[inline]
    pub fn element_count(&self) -> ElementCount {
        (self.element_size_and_count.get() >> 3) as uint
    }

    #[inline]
    pub fn inline_composite_word_count(&self) -> WordCount {
        self.element_count()
    }

    #[inline]
    pub fn set(&mut self, es : FieldSize, ec : ElementCount) {
        assert!(ec < (1 << 29), "Lists are limited to 2**29 elements");
        self.element_size_and_count.set(((ec as u32) << 3 ) | (es as u32));
    }

    #[inline]
    pub fn set_inline_composite(& mut self, wc : WordCount) {
        assert!(wc < (1 << 29), "Inline composite lists are limited to 2 ** 29 words");
        self.element_size_and_count.set((( wc as u32) << 3) | (INLINE_COMPOSITE as u32));
    }

}

impl WirePointer {

    #[inline]
    pub fn kind(&self) -> WirePointerKind {
        unsafe {
            std::cast::transmute((self.offset_and_kind.get() & 3) as u8)
        }
    }

    #[inline]
    pub fn target(&self) -> *Word {
        let thisAddr : *Word = unsafe {std::cast::transmute(&*self) };
        unsafe { thisAddr.offset(1 + ((self.offset_and_kind.get() as int) >> 2)) }
    }

    #[inline]
    pub fn mut_target(&mut self) -> *mut Word {
        let thisAddr : *mut Word = unsafe {std::cast::transmute(&*self) };
        unsafe { thisAddr.offset(1 + ((self.offset_and_kind.get() as int) >> 2)) }
    }

    #[inline]
    pub fn set_kind_and_target(&mut self, kind : WirePointerKind,
                            target : *mut Word, _segmentBuilder : *mut SegmentBuilder) {
        let thisAddr : int = unsafe {std::cast::transmute(&*self)};
        let targetAddr : int = unsafe {std::cast::transmute(target)};
        self.offset_and_kind.set(
            ((((targetAddr - thisAddr)/BYTES_PER_WORD as int) as i32 - 1) << 2) as u32
                | (kind as u32))
    }

    #[inline]
    pub fn set_kind_with_zero_offset(&mut self, kind : WirePointerKind) {
        self.offset_and_kind.set( kind as u32)
    }

    #[inline]
    pub fn inline_composite_list_element_count(&self) -> ElementCount {
        (self.offset_and_kind.get() >> 2) as ElementCount
    }

    #[inline]
    pub fn set_kind_and_inline_composite_list_element_count(
        &mut self, kind : WirePointerKind, element_count : ElementCount) {
        self.offset_and_kind.set((( element_count as u32 << 2) | (kind as u32)))
    }

    #[inline]
    pub fn far_position_in_segment(&self) -> WordCount {
        (self.offset_and_kind.get() >> 3) as WordCount
    }

    #[inline]
    pub fn is_double_far(&self) -> bool {
        ((self.offset_and_kind.get() >> 2) & 1) != 0
    }

    #[inline]
    pub fn set_far(&mut self, is_double_far : bool, pos : WordCount) {
        self.offset_and_kind.set
            (( pos << 3) as u32 | (is_double_far as u32 << 2) | WP_FAR as u32);
    }

    #[inline]
    pub fn struct_ref(&self) -> StructRef {
        unsafe { std::cast::transmute(self.upper32bits) }
    }

    #[inline]
    pub fn struct_ref_mut<'a>(&'a mut self) -> &'a mut StructRef {
        unsafe { std::cast::transmute(& self.upper32bits) }
    }

    #[inline]
    pub fn list_ref(&self) -> ListRef {
        unsafe { std::cast::transmute(self.upper32bits) }
    }

    #[inline]
    pub fn list_ref_mut<'a>(&'a self) -> &'a mut ListRef {
        unsafe { std::cast::transmute(& self.upper32bits) }
    }

    #[inline]
    pub fn far_ref(&self) -> FarRef {
        unsafe { std::cast::transmute(self.upper32bits) }
    }

    #[inline]
    pub fn far_ref_mut<'a>(&'a mut self) -> &'a mut FarRef {
        unsafe { std::cast::transmute(& self.upper32bits) }
    }

    #[inline]
    pub fn is_null(&self) -> bool {
        (self.offset_and_kind.get() == 0) & (self.upper32bits == 0)
    }
}

mod WireHelpers {
    use std;
    use common::*;
    use layout::*;
    use arena::*;
    use blob::*;

    #[inline]
    pub fn round_bytes_up_to_words(bytes : ByteCount) -> WordCount {
        //# This code assumes 64-bit words.
        (bytes + 7) / BYTES_PER_WORD
    }

    //# The maximum object size is 4GB - 1 byte. If measured in bits,
    //# this would overflow a 32-bit counter, so we need to accept
    //# BitCount64. However, 32 bits is enough for the returned
    //# ByteCounts and WordCounts.
    #[inline]
    pub fn round_bits_up_to_words(bits : BitCount64) -> WordCount {
        //# This code assumes 64-bit words.
        ((bits + 63) / (BITS_PER_WORD as u64)) as WordCount
    }

    #[inline]
    pub fn round_bits_up_to_bytes(bits : BitCount64) -> ByteCount {
        ((bits + 7) / (BITS_PER_BYTE as u64)) as ByteCount
    }

    #[inline]
    pub unsafe fn bounds_check<'a>(segment : *SegmentReader<'a>,
                                  start : *Word, end : *Word) -> bool {
        //# If segment is null, this is an unchecked message, so we don't do bounds checks.
        return segment.is_null() || (*segment).contains_interval(start, end);
    }

    #[inline]
    pub unsafe fn allocate(reff : &mut *mut WirePointer,
                           segment : &mut *mut SegmentBuilder,
                           amount : WordCount, kind : WirePointerKind) -> *mut Word {
        let is_null = (**reff).is_null();
        if (!is_null) {
            zero_object(*segment, *reff)
        }
        match (**segment).allocate(amount) {
            None => {

                //# Need to allocate in a new segment. We'll need to
                //# allocate an extra pointer worth of space to act as
                //# the landing pad for a far pointer.

                let amountPlusRef = amount + POINTER_SIZE_IN_WORDS;
                *segment = (*(**segment).messageBuilder).get_segment_with_available(amountPlusRef);
                let ptr : *mut Word = (**segment).allocate(amountPlusRef).unwrap();

                //# Set up the original pointer to be a far pointer to
                //# the new segment.
                (**reff).set_far(false, (**segment).get_word_offset_to(ptr));
                (**reff).far_ref_mut().segment_id.set((**segment).id);

                //# Initialize the landing pad to indicate that the
                //# data immediately follows the pad.
                *reff = std::cast::transmute(ptr);

                let ptr1 = ptr.offset(POINTER_SIZE_IN_WORDS as int);
                (**reff).set_kind_and_target(kind, ptr1, *segment);
                return ptr1;
            }
            Some(ptr) => {
                (**reff).set_kind_and_target(kind, ptr, *segment);
                return ptr;
            }
        }
    }

    #[inline]
    pub unsafe fn follow_fars<'a>(reff: &mut *WirePointer,
                                 refTarget: *Word,
                                 segment : &mut *SegmentReader<'a>) -> *Word {

        //# If the segment is null, this is an unchecked message,
        //# so there are no FAR pointers.
        if !(*segment).is_null() && (**reff).kind() == WP_FAR {
            *segment =
                (*(**segment).messageReader).get_segment_reader((**reff).far_ref().segment_id.get());

            let ptr : *Word = (**segment).get_start_ptr().offset(
                (**reff).far_position_in_segment() as int);

            let padWords : int = if ((**reff).is_double_far()) { 2 } else { 1 };
            assert!(bounds_check(*segment, ptr, ptr.offset(padWords)));

            let pad : *WirePointer = std::cast::transmute(ptr);

            if (!(**reff).is_double_far() ) {
                *reff = pad;
                return (*pad).target();
            } else {
                //# Landing pad is another far pointer. It is
                //# followed by a tag describing the pointed-to
                //# object.

                *reff = pad.offset(1);

                *segment =
                    (*(**segment).messageReader).get_segment_reader((*pad).far_ref().segment_id.get());

                return (**segment).get_start_ptr().offset((*pad).far_position_in_segment() as int);
            }
        } else {
            return refTarget;
        }
    }

    pub unsafe fn zero_object(mut segment : *mut SegmentBuilder, reff : *mut WirePointer) {
        //# Zero out the pointed-to object. Use when the pointer is
        //# about to be overwritten making the target object no longer
        //# reachable.

        match (*reff).kind() {
            WP_STRUCT | WP_LIST | WP_CAPABILITY => {
                zero_object_helper(segment,
                                 reff, (*reff).mut_target())
            }
            WP_FAR => {
                segment = std::ptr::to_mut_unsafe_ptr(
                    (*(*segment).messageBuilder).segment_builders[(*reff).far_ref().segment_id.get()]);
                let pad : *mut WirePointer =
                    std::cast::transmute((*segment).get_ptr_unchecked((*reff).far_position_in_segment()));

                if ((*reff).is_double_far()) {
                    segment = std::ptr::to_mut_unsafe_ptr(
                        (*(*segment).messageBuilder).segment_builders[(*pad).far_ref().segment_id.get()]);

                    zero_object_helper(segment,
                                     pad.offset(1),
                                     (*segment).get_ptr_unchecked((*pad).far_position_in_segment()));

                    std::ptr::set_memory(pad, 0u8, 2);

                } else {
                    zero_object(segment, pad);
                    std::ptr::set_memory(pad, 0u8, 1);
                }
            }
        }
    }

    pub unsafe fn zero_object_helper(segment : *mut SegmentBuilder,
                                     tag : *mut WirePointer,
                                     ptr: *mut Word) {
        match (*tag).kind() {
            WP_CAPABILITY => { fail!("Don't know how to handle CAPABILITY") }
            WP_STRUCT => {
                let pointerSection : *mut WirePointer =
                    std::cast::transmute(
                    ptr.offset((*tag).struct_ref().data_size.get() as int));

                let count = (*tag).struct_ref().ptr_count.get() as int;
                for i in range::<int>(0, count) {
                    zero_object(segment, pointerSection.offset(i));
                }
                std::ptr::set_memory(ptr, 0u8, (*tag).struct_ref().word_size());
            }
            WP_LIST => {
                match (*tag).list_ref().element_size() {
                    VOID =>  { }
                    BIT | BYTE | TWO_BYTES | FOUR_BYTES | EIGHT_BYTES => {
                        std::ptr::set_memory(
                            ptr, 0u8,
                            round_bits_up_to_words((
                                    (*tag).list_ref().element_count() *
                                        data_bits_per_element(
                                        (*tag).list_ref().element_size())) as u64))
                    }
                    POINTER => {
                        let count = (*tag).list_ref().element_count() as uint;
                        for i in range::<int>(0, count as int) {
                            zero_object(segment,
                                       std::cast::transmute(ptr.offset(i)))
                        }
                        std::ptr::set_memory(ptr, 0u8, count);
                    }
                    INLINE_COMPOSITE => {
                        let elementTag : *mut WirePointer = std::cast::transmute(ptr);

                        assert!((*elementTag).kind() == WP_STRUCT,
                                "Don't know how to handle non-STRUCT inline composite");

                        let data_size = (*elementTag).struct_ref().data_size.get();
                        let pointer_count = (*elementTag).struct_ref().ptr_count.get();
                        let mut pos : *mut Word = ptr.offset(1);
                        let count = (*elementTag).inline_composite_list_element_count();
                        for _ in range(0, count) {
                            pos = pos.offset(data_size as int);
                            for _ in range(0, pointer_count as uint) {
                                zero_object(
                                    segment,
                                    std::cast::transmute::<*mut Word, *mut WirePointer>(pos));
                                pos = pos.offset(1);
                            }
                        }
                        std::ptr::set_memory(ptr, 0u8,
                                             (*elementTag).struct_ref().word_size() * count + 1);
                    }
                }
            }
            WP_FAR => { fail!("Unexpected FAR pointer") }
        }
    }

    #[inline]
    pub unsafe fn init_struct_pointer(mut reff : *mut WirePointer,
                             mut segmentBuilder : *mut SegmentBuilder,
                             size : StructSize) -> StructBuilder {
        let ptr : *mut Word = allocate(&mut reff, &mut segmentBuilder, size.total(), WP_STRUCT);
        (*reff).struct_ref_mut().set(size);

        StructBuilder {
            segment : segmentBuilder,
            data : std::cast::transmute(ptr),
            pointers : std::cast::transmute(
                    ptr.offset((size.data as uint) as int)),
            data_size : size.data as WordCount32 * (BITS_PER_WORD as BitCount32),
            pointer_count : size.pointers,
            bit0offset : 0
        }
    }

    #[inline]
    pub unsafe fn get_writable_struct_pointer(_reff : *mut WirePointer,
                                    _segment : *mut SegmentBuilder,
                                    _size : StructSize,
                                    _defaultValue : *Word) -> StructBuilder {
        fail!("unimplemented")
    }

    #[inline]
    pub unsafe fn init_list_pointer(mut reff : *mut WirePointer,
                           mut segmentBuilder : *mut SegmentBuilder,
                           element_count : ElementCount,
                           element_size : FieldSize) -> ListBuilder {
        match element_size {
            INLINE_COMPOSITE => {
                fail!("Should have called initStructListPointer() instead")
            }
            _ => { }
        }

        let data_size : BitCount0 = data_bits_per_element(element_size);
        let pointer_count = pointers_per_element(element_size);
        let step = (data_size + pointer_count * BITS_PER_POINTER);
        let wordCount = round_bits_up_to_words(element_count as ElementCount64 * (step as u64));
        let ptr = allocate(&mut reff, &mut segmentBuilder, wordCount, WP_LIST);

        (*reff).list_ref_mut().set(element_size, element_count);

        ListBuilder {
            segment : segmentBuilder,
            ptr : std::cast::transmute(ptr),
            step : step,
            element_count : element_count,
            struct_data_size : data_size as u32,
            struct_pointer_count : pointer_count as u16
        }
    }

    #[inline]
    pub unsafe fn init_struct_list_pointer(mut reff : *mut WirePointer,
                                        mut segmentBuilder : *mut SegmentBuilder,
                                        element_count : ElementCount,
                                        element_size : StructSize) -> ListBuilder {
        match element_size.preferred_list_encoding {
            INLINE_COMPOSITE => { }
            otherEncoding => {
                return init_list_pointer(reff, segmentBuilder, element_count, otherEncoding);
            }
        }

        let wordsPerElement = element_size.total();

        //# Allocate the list, prefixed by a single WirePointer.
        let wordCount : WordCount = element_count * wordsPerElement;
        let ptr : *mut WirePointer =
            std::cast::transmute(allocate(&mut reff, &mut segmentBuilder,
                                          POINTER_SIZE_IN_WORDS + wordCount, WP_LIST));

        //# Initialize the pointer.
        (*reff).list_ref_mut().set_inline_composite(wordCount);
        (*ptr).set_kind_and_inline_composite_list_element_count(WP_STRUCT, element_count);
        (*ptr).struct_ref_mut().set(element_size);

        let ptr1 = ptr.offset(POINTER_SIZE_IN_WORDS as int);

        ListBuilder {
            segment : segmentBuilder,
            ptr : std::cast::transmute(ptr1),
            step : wordsPerElement * BITS_PER_WORD,
            element_count : element_count,
            struct_data_size : element_size.data as u32 * (BITS_PER_WORD as u32),
            struct_pointer_count : element_size.pointers
        }
    }

    #[inline]
    pub unsafe fn get_writable_list_pointer(_origRefIndex : *mut WirePointer,
                                         _origSegment : *mut SegmentBuilder,
                                         _element_size : FieldSize,
                                         _defaultValue : *Word) -> ListBuilder {
        fail!("unimplemented")
    }

    #[inline]
    pub unsafe fn get_writable_struct_list_pointer(_origRefIndex : *mut WirePointer,
                                               _origSegment : *mut SegmentBuilder,
                                               _element_size : StructSize,
                                               _defaultValue : *Word) -> ListBuilder {
        fail!("unimplemented")
    }

    #[inline]
    pub unsafe fn set_text_pointer(mut reff : *mut WirePointer,
                          mut segmentBuilder : *mut SegmentBuilder,
                          value : &str) {

        // initTextPointer is rolled in here

        let bytes : &[u8] = value.as_bytes();

        //# The byte list must include a NUL terminator
        let byteSize = bytes.len() + 1;

        let ptr =
            allocate(&mut reff, &mut segmentBuilder, round_bytes_up_to_words(byteSize), WP_LIST);

        (*reff).list_ref_mut().set(BYTE, byteSize);
        let dst : *mut u8 = std::cast::transmute(ptr);
        let src : *u8 = bytes.unsafe_ref(0);
        std::ptr::copy_nonoverlapping_memory(dst, src, bytes.len());

        // null terminate
        std::ptr::zero_memory(dst.offset(bytes.len() as int), 1);
    }

    #[inline]
    pub unsafe fn get_writable_text_pointer(_refIndex : *mut WirePointer,
                                         _segment : *mut SegmentBuilder,
                                         _defaultValue : &'static str) -> Text::Builder {
        fail!("unimplemented");
    }

    #[inline]
    pub unsafe fn read_struct_pointer<'a>(mut segment: *SegmentReader<'a>,
                                        mut reff : *WirePointer,
                                        defaultValue : *Word,
                                        nesting_limit : int) -> StructReader<'a> {

        if ((*reff).is_null()) {
            if (defaultValue.is_null() ||
                (*std::cast::transmute::<*Word,*WirePointer>(defaultValue)).is_null()) {
                    return StructReader::new_default();
            }

            //segment = std::ptr::null();
            //reff = std::cast::transmute::<*Word,*WirePointer>(defaultValue);
            fail!("default struct values unimplemented");
        }

        let refTarget : *Word = (*reff).target();

        assert!(nesting_limit > 0, "Message is too deeply-nested or contains cycles.");

        let ptr = follow_fars(&mut reff, refTarget, &mut segment);

        let data_size_words = (*reff).struct_ref().data_size.get();

        assert!(bounds_check(segment, ptr,
                            ptr.offset((*reff).struct_ref().word_size() as int)),
                "Message contained out-of-bounds struct pointer.");

        StructReader {segment : segment,
                      data : std::cast::transmute(ptr),
                      pointers : std::cast::transmute(ptr.offset(data_size_words as int)),
                      data_size : data_size_words as u32 * BITS_PER_WORD as BitCount32,
                      pointer_count : (*reff).struct_ref().ptr_count.get(),
                      bit0offset : 0,
                      nesting_limit : nesting_limit - 1 }
     }

    #[inline]
    pub unsafe fn read_list_pointer<'a>(mut segment: *SegmentReader<'a>,
                                      mut reff : *WirePointer,
                                      defaultValue : *Word,
                                      expectedElementSize : FieldSize,
                                      nesting_limit : int ) -> ListReader<'a> {

        if ((*reff).is_null()) {
            if defaultValue.is_null() ||
                (*std::cast::transmute::<*Word,*WirePointer>(defaultValue)).is_null() {
                return ListReader::new_default();
            }
            fail!("list default values unimplemented");
        }

        let refTarget : *Word = (*reff).target();

        if (nesting_limit <= 0) {
           fail!("nesting limit exceeded");
        }

        let mut ptr : *Word = follow_fars(&mut reff, refTarget, &mut segment);

        assert!((*reff).kind() == WP_LIST,
                "Message contains non-list pointer where list pointer was expected {:?}", reff);

        let list_ref = (*reff).list_ref();

        match list_ref.element_size() {
            INLINE_COMPOSITE => {
                let wordCount = list_ref.inline_composite_word_count();

                let tag: *WirePointer = std::cast::transmute(ptr);

                ptr = ptr.offset(1);

                assert!(bounds_check(segment, ptr.offset(-1),
                                    ptr.offset(wordCount as int)));

                assert!((*tag).kind() == WP_STRUCT,
                        "INLINE_COMPOSITE lists of non-STRUCT type are not supported");

                let size = (*tag).inline_composite_list_element_count();
                let struct_ref = (*tag).struct_ref();
                let wordsPerElement = struct_ref.word_size();

                assert!(size * wordsPerElement <= wordCount,
                       "INLINE_COMPOSITE list's elements overrun its word count");

                //# If a struct list was not expected, then presumably
                //# a non-struct list was upgraded to a struct list.
                //# We need to manipulate the pointer to point at the
                //# first field of the struct. Together with the
                //# "stepBits", this will allow the struct list to be
                //# accessed as if it were a primitive list without
                //# branching.

                //# Check whether the size is compatible.
                match expectedElementSize {
                    VOID => {}
                    BIT => fail!("Expected a bit list, but got a list of structs"),
                    BYTE | TWO_BYTES | FOUR_BYTES | EIGHT_BYTES => {
                        assert!(struct_ref.data_size.get() > 0,
                               "Expected a primitive list, but got a list of pointer-only structs")
                    }
                    POINTER => {
                        ptr = ptr.offset(struct_ref.data_size.get() as int);
                        assert!(struct_ref.ptr_count.get() > 0,
                               "Expected a pointer list, but got a list of data-only structs")
                    }
                    INLINE_COMPOSITE => {}
                }

                ListReader {
                    segment : segment,
                    ptr : std::cast::transmute(ptr),
                    element_count : size,
                    step : wordsPerElement * BITS_PER_WORD,
                    struct_data_size : struct_ref.data_size.get() as u32 * (BITS_PER_WORD as u32),
                    struct_pointer_count : struct_ref.ptr_count.get() as u16,
                    nesting_limit : nesting_limit - 1
                }
            }
            _ => {

                //# This is a primitive or pointer list, but all such
                //# lists can also be interpreted as struct lists. We
                //# need to compute the data size and pointer count for
                //# such structs.
                let data_size = data_bits_per_element(list_ref.element_size());
                let pointer_count = pointers_per_element(list_ref.element_size());
                let step = data_size + pointer_count * BITS_PER_POINTER;

                assert!(
                    bounds_check(
                        segment, ptr,
                        ptr.offset(
                            round_bits_up_to_words(
                                (list_ref.element_count() * step) as u64) as int)));

                //# Verify that the elements are at least as large as
                //# the expected type. Note that if we expected
                //# INLINE_COMPOSITE, the expected sizes here will be
                //# zero, because bounds checking will be performed at
                //# field access time. So this check here is for the
                //# case where we expected a list of some primitive or
                //# pointer type.

                let expectedDataBitsPerElement =
                        data_bits_per_element(expectedElementSize);
                let expectedPointersPerElement =
                    pointers_per_element(expectedElementSize);

                assert!(expectedDataBitsPerElement <= data_size);
                assert!(expectedPointersPerElement <= pointer_count);

                ListReader {
                    segment : segment,
                    ptr : std::cast::transmute(ptr),
                    element_count : list_ref.element_count(),
                    step : step,
                    struct_data_size : data_size as u32,
                    struct_pointer_count : pointer_count as u16,
                    nesting_limit : nesting_limit - 1
                }
            }
        }

    }

    #[inline]
    pub unsafe fn read_text_pointer<'a>(mut segment : *SegmentReader<'a>,
                                      mut reff : *WirePointer,
                                      defaultValue : &'a str
                                      //defaultSize : ByteCount
                                      ) -> Text::Reader<'a> {
        if (reff.is_null() || (*reff).is_null()) {
            return defaultValue;
        }

        let refTarget = (*reff).target();

        let ptr : *Word = follow_fars(&mut reff, refTarget, &mut segment);

        let list_ref = (*reff).list_ref();

        let size : uint = list_ref.element_count();

        assert!((*reff).kind() == WP_LIST,
                "Message contains non-list pointer where text was expected");

        assert!(list_ref.element_size() == BYTE);

        assert!(bounds_check(segment, ptr,
                            ptr.offset(round_bytes_up_to_words(size) as int)));

        assert!(size > 0, "Message contains text that is not NUL-terminated");

        let strPtr = std::cast::transmute::<*Word,*i8>(ptr);

        assert!((*strPtr.offset((size - 1) as int)) == 0i8,
                "Message contains text that is not NUL-terminated");

        std::str::raw::c_str_to_static_slice(strPtr)
    }
}

static zero : u64 = 0;
fn zero_pointer() -> *WirePointer { unsafe {std::cast::transmute(std::ptr::to_unsafe_ptr(&zero))}}

pub struct PointerReader<'a> {
    segment : *SegmentReader<'a>,
    pointer : *WirePointer,
    nesting_limit : int
}

impl <'a> PointerReader<'a> {
    pub fn new_default<'b>() -> PointerReader<'b> {
        PointerReader { segment : std::ptr::null(),
                        pointer : std::ptr::null(),
                        nesting_limit : 0x7fffffff }
    }

    pub fn is_null(&self) -> bool {
        self.pointer.is_null() || unsafe { (*self.pointer).is_null() }
    }

    pub fn get_struct(&self) -> StructReader<'a> {
        let reff : *WirePointer = if self.pointer.is_null() { zero_pointer() } else { self.pointer };
        unsafe {
            WireHelpers::read_struct_pointer(self.segment, reff,
                                             std::ptr::null(), self.nesting_limit)
        }
    }

    pub fn get_list(&self, expected_element_size : FieldSize) -> ListReader<'a> {
        let reff = if self.pointer.is_null() { zero_pointer() } else { self.pointer };
        unsafe {
            WireHelpers::read_list_pointer(self.segment,
                                           reff,
                                           std::ptr::null(),
                                           expected_element_size, self.nesting_limit)
        }
    }
}

pub struct PointerBuilder {
    segment : *mut SegmentBuilder,
    pointer : *mut WirePointer
}

impl PointerBuilder {
    pub fn is_null(&self) -> bool {
        unsafe { (*self.pointer).is_null() }
    }
}

pub trait FromStructReader<'a> {
    fn from_struct_reader(reader : StructReader<'a>) -> Self;
}

pub struct StructReader<'a> {
    segment : *SegmentReader<'a>,
    data : *u8,
    pointers : *WirePointer,
    data_size : BitCount32,
    pointer_count : WirePointerCount16,
    bit0offset : BitCount8,
    nesting_limit : int
}

impl <'a> StructReader<'a>  {

    pub fn new_default() -> StructReader {
        StructReader { segment : std::ptr::null(),
                       data : std::ptr::null(),
                       pointers : std::ptr::null(), data_size : 0, pointer_count : 0,
                       bit0offset : 0, nesting_limit : 0x7fffffff}
    }

    pub fn read_root<'b>(location : WordCount, segment : *SegmentReader<'b>,
                         nesting_limit : int) -> StructReader<'b> {
        //  the pointer to the struct is at segment[location]
        unsafe {
            // TODO bounds check
            let reff : *WirePointer =
                std::cast::transmute((*segment).segment.unsafe_ref(location));

            WireHelpers::read_struct_pointer(segment, reff, std::ptr::null(), nesting_limit)
        }
    }

    pub fn get_data_section_size(&self) -> BitCount32 { self.data_size }

    pub fn get_pointer_section_size(&self) -> WirePointerCount16 { self.pointer_count }

    pub fn get_data_section_as_blob(&self) -> uint { fail!("unimplemented") }

    #[inline]
    pub fn get_data_field<T:Clone + std::num::Zero>(&self, offset : ElementCount) -> T {
        // We need to check the offset because the struct may have
        // been created with an old version of the protocol that did
        // not contain the field.
        if ((offset + 1) * bitsPerElement::<T>() <= self.data_size as uint) {
            unsafe {
                let dwv : *WireValue<T> = std::cast::transmute(self.data);
                (*dwv.offset(offset as int)).get()
            }
        } else {
            return std::num::Zero::zero()
        }
    }


    #[inline]
    pub fn get_bool_field(&self, offset : ElementCount) -> bool {
        let mut boffset : BitCount32 = offset as BitCount32;
        if (boffset < self.data_size) {
            if (offset == 0) {
                boffset = self.bit0offset as BitCount32;
            }
            unsafe {
                let b : *u8 = self.data.offset((boffset as uint / BITS_PER_BYTE) as int);
                ((*b) & (1 << (boffset % BITS_PER_BYTE as u32 ))) != 0
            }
        } else {
            false
        }
    }

    #[inline]
    pub fn get_data_field_mask<T:Clone + std::num::Zero + Mask>(&self,
                                                                offset : ElementCount,
                                                                mask : T) -> T {
        Mask::mask(self.get_data_field(offset), mask)
    }

    #[inline]
    pub fn get_bool_field_mask(&self,
                               offset : ElementCount,
                               mask : bool) -> bool {
       self.get_bool_field(offset) ^ mask
    }

    #[inline]
    pub fn get_pointer_field(&self, ptr_index : WirePointerCount) -> PointerReader<'a> {
        if (ptr_index < self.pointer_count as WirePointerCount) {
            PointerReader {
                segment : self.segment,
                pointer : unsafe { self.pointers.offset(ptr_index as int) },
                nesting_limit : self.nesting_limit
            }
        } else {
            PointerReader::new_default()
        }
    }

    pub fn get_struct_field(&self, ptrIndex : WirePointerCount, _defaultValue : Option<&'a [u8]>)
        -> StructReader<'a> {
        let reff : *WirePointer = if (ptrIndex >= self.pointer_count as WirePointerCount)
            { std::ptr::null() }
        else
            { unsafe { self.pointers.offset(ptrIndex as int)} };

        unsafe {
            WireHelpers::read_struct_pointer(self.segment, reff,
                                           std::ptr::null(), self.nesting_limit)
        }
    }

    pub fn get_list_field(&self,
                          ptrIndex : WirePointerCount, expectedElementSize : FieldSize,
                          _defaultValue : Option<&'a [u8]>) -> ListReader<'a> {
        let reff : *WirePointer =
            if (ptrIndex >= self.pointer_count as WirePointerCount)
            { std::ptr::null() }
            else { unsafe{ self.pointers.offset(ptrIndex as int )} };

        unsafe {
            WireHelpers::read_list_pointer(self.segment,
                                           reff,
                                           std::ptr::null(),
                                           expectedElementSize, self.nesting_limit)
        }
    }

    pub fn get_text_field(&self, ptrIndex : WirePointerCount,
                        defaultValue : &'a str) -> Text::Reader<'a> {
        let reff : *WirePointer =
            if (ptrIndex >= self.pointer_count as WirePointerCount) {
                std::ptr::null()
            } else {
                unsafe{self.pointers.offset(ptrIndex as int)}
            };
        unsafe {
            WireHelpers::read_text_pointer(self.segment, reff, defaultValue)
        }
    }

    pub fn total_size(&self) -> WordCount64 {
        fail!("total_size is unimplemented");
    }

}

pub trait HasStructSize {
    fn struct_size(unused_self : Option<Self>) -> StructSize;
}

pub trait FromStructBuilder {
    fn from_struct_builder(structBuilder : StructBuilder) -> Self;
}

pub struct StructBuilder {
    segment : *mut SegmentBuilder,
    data : *mut u8,
    pointers : *mut WirePointer,
    data_size : BitCount32,
    pointer_count : WirePointerCount16,
    bit0offset : BitCount8
}

impl StructBuilder {
    pub fn as_reader<T>(&self, f : |StructReader| -> T) -> T {
        unsafe {
            (*self.segment).as_reader( |segmentReader| {
                f ( StructReader {
                        segment : std::ptr::to_unsafe_ptr(segmentReader),
                        data : std::cast::transmute(self.data),
                        pointers : std::cast::transmute(self.pointers),
                        data_size : self.data_size,
                        pointer_count : self.pointer_count,
                        bit0offset : self.bit0offset,
                        nesting_limit : 0x7fffffff
                    })
            })
        }
    }

    pub fn init_root(segment : *mut SegmentBuilder,
                     location : *mut WirePointer,
                     size : StructSize) -> StructBuilder {
        unsafe {
            WireHelpers::init_struct_pointer(location, segment, size)
        }
    }

    #[inline]
    pub fn set_data_field<T:Clone>(&self, offset : ElementCount, value : T) {
        unsafe {
            let ptr : *mut WireValue<T> = std::cast::transmute(self.data);
            (*ptr.offset(offset as int)).set(value)
        }
    }

    #[inline]
    pub fn get_data_field<T:Clone>(&self, offset : ElementCount) -> T {
        unsafe {
            let ptr : *mut WireValue<T> = std::cast::transmute(self.data);
            (*ptr.offset(offset as int)).get()
        }
    }

    #[inline]
    pub fn set_bool_field(&self, offset : ElementCount, value : bool) {
        //# This branch should be compiled out whenever this is
        //# inlined with a constant offset.
        let boffset : BitCount0 = if (offset == 0) { self.bit0offset as uint } else { offset };
        let b = unsafe { self.data.offset((boffset / BITS_PER_BYTE) as int)};
        let bitnum = boffset % BITS_PER_BYTE;
        unsafe { (*b) = (( (*b) & !(1 << bitnum)) | (value as u8 << bitnum)) }
    }

    #[inline]
    pub fn get_bool_field(&self, offset : ElementCount) -> bool {
        let boffset : BitCount0 =
            if (offset == 0) {self.bit0offset as BitCount0} else {offset};
        let b = unsafe { self.data.offset((boffset / BITS_PER_BYTE) as int) };
        unsafe { ((*b) & (1 << (boffset % BITS_PER_BYTE ))) != 0 }
    }

    //# Initializes the struct field at the given index in the pointer
    //# section. If it is already initialized, the previous value is
    //# discarded or overwritten. The struct is initialized to the type's
    //# default state (all-zero). Use getStructField() if you want the
    //# struct to be initialized as a copy of the field's default value
    //# (which may have non-null pointers).
    pub fn init_struct_field(&self, ptrIndex : WirePointerCount, size : StructSize)
        -> StructBuilder {
        unsafe {
            WireHelpers::init_struct_pointer(self.pointers.offset(ptrIndex as int),
                                             self.segment, size)
        }
    }

    //# Gets the struct field at the given index in the pointer
    //# section. If the field is not already initialized, it is
    //# initialized as a deep copy of the given default value (a flat
    //# message), or to the empty state if defaultValue is nullptr.
    pub fn get_struct_field(&self, ptrIndex : WirePointerCount, size : StructSize,
                            _defaultValue : Option<()>) -> StructBuilder {
        unsafe {
            WireHelpers::get_writable_struct_pointer(
                self.pointers.offset(ptrIndex as int),
                self.segment,
                size,
                std::ptr::null())
        }
    }

    //# Allocates a new list of the given size for the field at the given
    //# index in the pointer segment, and return a pointer to it. All
    //# elements are initialized to zero.
    pub fn init_list_field(&self, ptrIndex : WirePointerCount,
                           element_size : FieldSize, element_count : ElementCount)
        -> ListBuilder {
        unsafe {
            WireHelpers::init_list_pointer(
                self.pointers.offset(ptrIndex as int),
                self.segment, element_count, element_size)
        }
    }

    //# Gets the already-allocated list field for the given pointer
    //# index, ensuring that the list is suitable for storing
    //# non-struct elements of the given size. If the list is not
    //# already allocated, it is allocated as a deep copy of the given
    //# default value (a flat message). If the default value is null,
    //# an empty list is used.
    pub fn get_list_field(&self, ptrIndex : WirePointerCount,
                          element_size : FieldSize, _defaultValue : Option<()>) -> ListBuilder {
        unsafe {
            WireHelpers::get_writable_list_pointer(
                self.pointers.offset(ptrIndex as int),
                self.segment, element_size, std::ptr::null())
        }
    }

    //# Allocates a new list of the given size for the field at the
    //# given index in the pointer segment, and return a pointer to it.
    //# Each element is initialized to its empty state.
    pub fn init_struct_list_field(&self, ptrIndex : WirePointerCount,
                                  element_count : ElementCount, element_size : StructSize)
        -> ListBuilder {
        unsafe { WireHelpers::init_struct_list_pointer(
                self.pointers.offset(ptrIndex as int),
                self.segment, element_count, element_size)
        }
    }

    //# Gets the already-allocated list field for the given pointer
    //# index, ensuring that the list is suitable for storing struct
    //# elements of the given size. If the list is not already
    //# allocated, it is allocated as a deep copy of the given default
    //# value (a flat message). If the default value is null, an empty
    //# list is used.
    pub fn get_struct_list_field(&self, ptrIndex : WirePointerCount,
                                 element_size : StructSize,
                                 _defaultValue : Option<()>) -> ListBuilder {
        unsafe {
            WireHelpers::get_writable_struct_list_pointer(
                self.pointers.offset(ptrIndex as int),
                self.segment, element_size,
                std::ptr::null())
        }
    }

    pub fn set_text_field(&self, ptrIndex : WirePointerCount, value : &str) {
        unsafe {
            WireHelpers::set_text_pointer(
                self.pointers.offset(ptrIndex as int),
                self.segment, value)
        }
    }


    pub fn get_text_field(&self, ptrIndex : WirePointerCount,
                          defaultValue : &'static str) -> Text::Builder {
        unsafe {
            WireHelpers::get_writable_text_pointer(
                self.pointers.offset(ptrIndex as int),
                self.segment, defaultValue)
        }
    }

}

pub struct ListReader<'a> {
    segment : *SegmentReader<'a>,
    ptr : *u8,
    element_count : ElementCount,
    step : BitCount0,
    struct_data_size : BitCount32,
    struct_pointer_count : WirePointerCount16,
    nesting_limit : int
}

impl <'a> ListReader<'a> {

    pub fn new_default() -> ListReader {
        ListReader { segment : std::ptr::null(),
                    ptr : std::ptr::null(), element_count : 0, step: 0, struct_data_size : 0,
                    struct_pointer_count : 0, nesting_limit : 0x7fffffff}
    }

    #[inline]
    pub fn size(&self) -> ElementCount { self.element_count }

    pub fn get_struct_element(&self, index : ElementCount) -> StructReader<'a> {
        assert!(self.nesting_limit > 0,
                "Message is too deeply-nested or contains cycles");
        let indexBit : BitCount64 = index as ElementCount64 * (self.step as BitCount64);

        let structData : *u8 = unsafe {
            self.ptr.offset((indexBit as uint / BITS_PER_BYTE) as int) };

        let structPointers : *WirePointer = unsafe {
                std::cast::transmute(
                    structData.offset((self.struct_data_size as uint / BITS_PER_BYTE) as int))
        };

/*
        assert!(self.struct_pointer_count == 0 ||
                structPointers % BYTES_PER_POINTER == 0,
                "Pointer section of struct list element not aligned"
               );
*/
        StructReader {
            segment : self.segment,
            data : structData,
            pointers : structPointers,
            data_size : self.struct_data_size as BitCount32,
            pointer_count : self.struct_pointer_count,
            bit0offset : (indexBit % (BITS_PER_BYTE as u64)) as u8,
            nesting_limit : self.nesting_limit - 1
        }
    }

    pub fn get_list_element(&self, _index : ElementCount, _expectedElementSize : FieldSize)
        -> ListReader<'a> {
        fail!("unimplemented")
    }
}


pub struct ListBuilder {
    segment : *mut SegmentBuilder,
    ptr : *mut u8,
    element_count : ElementCount,
    step : BitCount0,
    struct_data_size : BitCount32,
    struct_pointer_count : WirePointerCount16
}

impl ListBuilder {

    #[inline]
    pub fn size(&self) -> ElementCount { self.element_count }

    pub fn get_struct_element(&self, index : ElementCount) -> StructBuilder {
        let indexBit = index * self.step;
        let structData = unsafe{ self.ptr.offset((indexBit / BITS_PER_BYTE) as int)};
        let structPointers = unsafe {
            std::cast::transmute(
                structData.offset(((self.struct_data_size as uint) / BITS_PER_BYTE) as int))
        };
        StructBuilder {
            segment : self.segment,
            data : structData,
            pointers : structPointers,
            data_size : self.struct_data_size,
            pointer_count : self.struct_pointer_count,
            bit0offset : (indexBit % BITS_PER_BYTE) as u8
        }
    }
}


pub trait PrimitiveElement : Clone {
    #[inline]
    fn get(listReader : &ListReader, index : ElementCount) -> Self {
        unsafe {
            let ptr : *u8 =
                listReader.ptr.offset(
                                 (index * listReader.step / BITS_PER_BYTE) as int);
            (*std::cast::transmute::<*u8,*WireValue<Self>>(ptr)).get()
        }
    }

    #[inline]
    fn get_from_builder(listBuilder : &ListBuilder, index : ElementCount) -> Self {
        unsafe {
            let ptr : *mut WireValue<Self> =
                std::cast::transmute(
                listBuilder.ptr.offset(
                                     (index * listBuilder.step / BITS_PER_BYTE) as int));
            (*ptr).get()
        }
    }

    #[inline]
    fn set(listBuilder : &ListBuilder, index : ElementCount, value: Self) {
        unsafe {
            let ptr : *mut WireValue<Self> =
                std::cast::transmute(
                listBuilder.ptr.offset(
                    (index * listBuilder.step / BITS_PER_BYTE) as int));
            (*ptr).set(value);
        }
    }
}

impl PrimitiveElement for u8 { }
impl PrimitiveElement for u16 { }
impl PrimitiveElement for u32 { }
impl PrimitiveElement for u64 { }
impl PrimitiveElement for i8 { }
impl PrimitiveElement for i16 { }
impl PrimitiveElement for i32 { }
impl PrimitiveElement for i64 { }
impl PrimitiveElement for f32 { }
impl PrimitiveElement for f64 { }

impl PrimitiveElement for bool {
    #[inline]
    fn get(list : &ListReader, index : ElementCount) -> bool {
        //# Ignore stepBytes for bit lists because bit lists cannot be
        //# upgraded to struct lists.
        let bindex : BitCount0 = index * list.step;
        unsafe {
            let b : *u8 = list.ptr.offset((bindex / BITS_PER_BYTE) as int);
            ((*b) & (1 << (bindex % BITS_PER_BYTE))) != 0
        }
    }
    #[inline]
    fn get_from_builder(list : &ListBuilder, index : ElementCount) -> bool {
        //# Ignore stepBytes for bit lists because bit lists cannot be
        //# upgraded to struct lists.
        let bindex : BitCount0 = index * list.step;
        let b = unsafe { list.ptr.offset((bindex / BITS_PER_BYTE) as int) };
        unsafe { ((*b) & (1 << (bindex % BITS_PER_BYTE ))) != 0 }
    }
    #[inline]
    fn set(list : &ListBuilder, index : ElementCount, value : bool) {
        //# Ignore stepBytes for bit lists because bit lists cannot be
        //# upgraded to struct lists.
        let bindex : BitCount0 = index;
        let b = unsafe { list.ptr.offset((bindex / BITS_PER_BYTE) as int) };

        let bitnum = bindex % BITS_PER_BYTE;
        unsafe { (*b) = (( (*b) & !(1 << bitnum)) | (value as u8 << bitnum)) }
    }
}

impl PrimitiveElement for () {
    #[inline]
    fn get(_list : &ListReader, _index : ElementCount) -> () { () }

    #[inline]
    fn get_from_builder(_list : &ListBuilder, _index : ElementCount) -> () { () }

    #[inline]
    fn set(_list : &ListBuilder, _index : ElementCount, _value : ()) { }
}

