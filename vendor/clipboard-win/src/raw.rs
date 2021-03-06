//!Raw bindings to Windows clipboard.
//!
//!## General information
//!
//!All pre & post conditions are stated in description of functions.
//!
//!### Open clipboard
//! To access any information inside clipboard it is necessary to open it by means of
//! [open()](fn.open.html).
//!
//! After that Clipboard cannot be opened any more until [close()](fn.close.html) is called.

use ::std;
use std::cmp;
use std::os::windows::ffi::OsStrExt;
use std::os::raw::{
    c_int,
    c_uint
};
use std::ptr;
use std::io;

use ::utils;
use ::formats;

use winapi::basetsd::{
    SIZE_T
};

use kernel32::{
    GlobalSize,
    GlobalLock,
    GlobalUnlock,
    GlobalAlloc,
    GlobalFree
};

use user32::{
    OpenClipboard,
    CloseClipboard,
    EmptyClipboard,
    GetClipboardSequenceNumber,
    CountClipboardFormats,
    IsClipboardFormatAvailable,
    EnumClipboardFormats,
    RegisterClipboardFormatW,
    GetClipboardFormatNameW,
    GetClipboardData,
    SetClipboardData
};

#[inline]
///Opens clipboard.
///
///Wrapper around ```OpenClipboard```.
///
///# Pre-conditions:
///
///* Clipboard is not opened yet.
///
///# Post-conditions:
///
///* Clipboard can be accessed for read and write operations.
pub fn open() -> io::Result<()> {
    unsafe {
        if OpenClipboard(ptr::null_mut()) == 0 {
            return Err(utils::get_last_error());
        }
    }

    Ok(())
}

#[inline]
///Closes clipboard.
///
///Wrapper around ```CloseClipboard```.
///
///# Pre-conditions:
///
///* [open()](fn.open.html) has been called.
pub fn close() -> io::Result<()> {
    unsafe {
        if CloseClipboard() == 0 {
            return Err(utils::get_last_error());
        }
    }

    Ok(())
}

#[inline]
///Empties clipboard.
///
///Wrapper around ```EmptyClipboard```.
///
///# Pre-conditions:
///
///* [open()](fn.open.html) has been called.
pub fn empty() -> io::Result<()> {
    unsafe {
        if EmptyClipboard() == 0 {
            return Err(utils::get_last_error());
        }
    }

    Ok(())
}

#[inline]
///Retrieves clipboard sequence number.
///
///Wrapper around ```GetClipboardSequenceNumber```.
///
///# Returns:
///
///* ```Some``` Contains return value of ```GetClipboardSequenceNumber```.
///* ```None``` In case if you do not have access. It means that zero is returned by system.
pub fn seq_num() -> Option<u32> {
    let result: u32 = unsafe { GetClipboardSequenceNumber() };

    if result == 0 {
        return None;
    }

    Some(result)
}

#[inline]
///Retrieves size of clipboard data for specified format.
///
///# Pre-conditions:
///
///* [open()](fn.open.html) has been called.
///
///# Returns:
///
///Size in bytes if format presents on clipboard.
pub fn size(format: u32) -> Option<usize> {
    let clipboard_data = unsafe {GetClipboardData(format)};

    if clipboard_data.is_null() {
        None
    }
    else {
        unsafe {
            Some(GlobalSize(clipboard_data) as usize)
        }
    }
}

///Retrieves data of specified format from clipboard.
///
///Wrapper around ```GetClipboardData```.
///
///# Pre-conditions:
///
///* [open()](fn.open.html) has been called.
///
///# Note:
///
///Clipboard data is truncated by the size of provided storage.
///
///# Returns:
///
///Number of copied bytes.
pub fn get(format: u32, result: &mut [u8]) -> io::Result<usize> {
    let clipboard_data = unsafe { GetClipboardData(format as c_uint) };

    if clipboard_data.is_null() {
        Err(utils::get_last_error())
    }
    else {
        unsafe {
            let data_ptr = GlobalLock(clipboard_data) as *const u8;

            if data_ptr.is_null() {
                return Err(utils::get_last_error());
            }

            let data_size = cmp::min(GlobalSize(clipboard_data) as usize, result.len());

            ptr::copy_nonoverlapping(data_ptr, result.as_mut_ptr(), data_size);
            GlobalUnlock(clipboard_data);

            Ok(data_size)
        }
    }
}

///Retrieves String from `CF_UNICODETEXT` format
///
///Specialized version of [get](fn.get.html) to avoid unnecessary allocations.
///
///# Note:
///
///Usually WinAPI returns strings with null terminated character at the end.
///This character is trimmed.
///
///# Pre-conditions:
///
///* [open()](fn.open.html) has been called.
pub fn get_string() -> io::Result<String> {
    let clipboard_data = unsafe { GetClipboardData(formats::CF_UNICODETEXT) };

    if clipboard_data.is_null() {
        Err(utils::get_last_error())
    }
    else {
        unsafe {
            let data_ptr = GlobalLock(clipboard_data) as *const u16;

            if data_ptr.is_null() {
                return Err(utils::get_last_error());
            }

            let data_size = GlobalSize(clipboard_data) as usize / std::mem::size_of::<u16>();

            let str_slice = std::slice::from_raw_parts(data_ptr, data_size);
            let mut result = String::from_utf16_lossy(str_slice);

            {
                //It seems WinAPI always supposed to have at the end null char.
                //But just to be safe let's check for it and only then remove.
                if let Some(last) = result.pop() {
                    if last != '\0' {
                        result.push(last);
                    }
                }
            }

            GlobalUnlock(clipboard_data);

            Ok(result)
        }
    }
}

///Sets data onto clipboard with specified format.
///
///Wrapper around ```SetClipboardData```.
///
///# Pre-conditions:
///
///* [open()](fn.open.html) has been called.
pub fn set(format: u32, data: &[u8]) -> io::Result<()> {
    const GHND: c_uint = 0x42;
    let size = data.len();

    let alloc_handle = unsafe { GlobalAlloc(GHND, size as SIZE_T) };

    if alloc_handle.is_null() {
        Err(utils::get_last_error())
    }
    else {
        unsafe {
            let lock = GlobalLock(alloc_handle) as *mut u8;

            ptr::copy_nonoverlapping(data.as_ptr(), lock, size);
            GlobalUnlock(alloc_handle);
            EmptyClipboard();

            if SetClipboardData(format, alloc_handle).is_null() {
                let result = utils::get_last_error();
                GlobalFree(alloc_handle);
                Err(result)
            }
            else {
                Ok(())
            }
        }
    }
}

#[inline(always)]
///Determines whenever provided clipboard format is available on clipboard or not.
pub fn is_format_avail(format: u32) -> bool {
    unsafe { IsClipboardFormatAvailable(format) != 0 }
}

#[inline]
///Retrieves number of currently available formats on clipboard.
pub fn count_formats() -> io::Result<i32> {
    let result = unsafe { CountClipboardFormats() };

    if result == 0 {
        let error = utils::get_last_error();

        if let Some(raw_error) = error.raw_os_error() {
            if raw_error != 0 {
                return Err(error)
            }
        }
    }

    Ok(result)
}

///Enumerator over available clipboard formats.
///
///# Pre-conditions:
///
///* [open()](fn.open.html) has been called.
pub struct EnumFormats {
    idx: u32
}

impl EnumFormats {
    /// Constructs enumerator over all available formats.
    pub fn new() -> EnumFormats {
        EnumFormats { idx: 0 }
    }

    /// Constructs enumerator that starts from format.
    pub fn from(format: u32) -> EnumFormats {
        EnumFormats { idx: format }
    }

    /// Resets enumerator to list all available formats.
    pub fn reset(&mut self) -> &EnumFormats {
        self.idx = 0;
        self
    }
}

impl Iterator for EnumFormats {
    type Item = u32;

    /// Returns next format on clipboard.
    ///
    /// In case of failure (e.g. clipboard is closed) returns `None`.
    fn next(&mut self) -> Option<u32> {
        self.idx = unsafe { EnumClipboardFormats(self.idx) };

        if self.idx == 0 {
            None
        }
        else {
            Some(self.idx)
        }
    }

    /// Relies on `count_formats` so it is only reliable
    /// when hinting size for enumeration of all formats.
    ///
    /// Doesn't require opened clipboard.
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, count_formats().ok().map(|val| val as usize))
    }
}

macro_rules! match_format_name {
    ( $name:expr, $( $f:ident ),* ) => {
        match $name {
            $( formats::$f => Some(stringify!($f).to_string()),)*
            formats::CF_GDIOBJFIRST ... formats::CF_GDIOBJLAST => Some(format!("CF_GDIOBJ{}", $name - formats::CF_GDIOBJFIRST)),
            formats::CF_PRIVATEFIRST ... formats::CF_PRIVATELAST => Some(format!("CF_PRIVATE{}", $name - formats::CF_PRIVATEFIRST)),
            _ => {
                let format_buff = [0u16; 52];
                unsafe {
                    let buff_p = format_buff.as_ptr() as *mut u16;

                    if GetClipboardFormatNameW($name, buff_p, format_buff.len() as c_int) == 0 {
                        None
                    }
                    else {
                        Some(String::from_utf16_lossy(&format_buff))
                    }
                }
            }
        }
    }
}

///Returns format name based on it's code.
///
///# Parameters:
///
///* ```format``` clipboard format code.
///
///# Return result:
///
///* ```Some``` Name of valid format.
///* ```None``` Format is invalid or doesn't exist.
pub fn format_name(format: u32) -> Option<String> {
    match_format_name!(format,
                       CF_BITMAP,
                       CF_DIB,
                       CF_DIBV5,
                       CF_DIF,
                       CF_DSPBITMAP,
                       CF_DSPENHMETAFILE,
                       CF_DSPMETAFILEPICT,
                       CF_DSPTEXT,
                       CF_ENHMETAFILE,
                       CF_HDROP,
                       CF_LOCALE,
                       CF_METAFILEPICT,
                       CF_OEMTEXT,
                       CF_OWNERDISPLAY,
                       CF_PALETTE,
                       CF_PENDATA,
                       CF_RIFF,
                       CF_SYLK,
                       CF_TEXT,
                       CF_WAVE,
                       CF_TIFF,
                       CF_UNICODETEXT)
}

///Registers a new clipboard format with specified name.
///
///# Returns:
///
///Newly registered format identifier.
///
///# Note:
///
///Custom format identifier is in range `0xC000...0xFFFF`.
pub fn register_format<T: ?Sized + AsRef<std::ffi::OsStr>>(name: &T) -> io::Result<u32> {
    let mut utf16_buff: Vec<u16> = name.as_ref().encode_wide().collect();
    utf16_buff.push(0);

    let result = unsafe { RegisterClipboardFormatW(utf16_buff.as_ptr()) };

    if result == 0 {
        Err(utils::get_last_error())
    }
    else {
        Ok(result)
    }
}
