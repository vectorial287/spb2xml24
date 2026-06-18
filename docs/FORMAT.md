# SPB binary format

This document describes the compiled `SimBase` property format (`.spb`) as used
by Microsoft Flight Simulator 2024, to the extent needed to decompile it back to
XML. All multi byte integers are little endian.

## File header

| Offset | Type | Meaning |
| --- | --- | --- |
| 0 | `u16` | File id, always `60332` (`0xEBAC`). |
| 2 | `i32` x 12 | Header table. Entry 6 is the tag count. |

Only entry 6 of the header is needed for decoding. The remaining entries hold
version and bookkeeping values that do not affect the document structure.

## Tag table

The tag table follows the header and has `tag_count - 1` records. Each record is:

| Type | Meaning |
| --- | --- |
| `byte` x 16 | Property, set or type GUID. |
| `i32` | Value size in bytes, or `-1` for a variable length value. |

The 16 byte GUID uses the .NET `Guid(byte[16])` layout: the first three fields
are little endian, the remaining eight bytes are in order. Each GUID is resolved
against the property definitions to find a name and a value type.

The value size is the key addition that the 2024 format relies on. It records
the exact on disk size of the property value. For most types this simply
confirms the size implied by the type. For float properties it also signals
whether the value is a plain float or an input pin (see below).

## Document body

The body is a depth first sequence of elements. Each element begins with an
`i32` tag value. A tag of `0` marks the absence of an element. Otherwise the tag
minus one is an index into the tag table.

A resolved tag is either a set or a property.

### Sets

A set element is:

| Type | Meaning |
| --- | --- |
| `i32` | Body length in bytes. |
| ... | Child elements until the body length is consumed. |

The set's element name is the set name, qualified with its owning symbol when
that symbol differs from the parent context (for example `WorldBase.Flight`).
Child elements inherit the set's symbol as their context.

### Properties

A property's value immediately follows its tag. The value is decoded according
to the declared type. Fixed size types and their byte sizes:

| Type | Size | Rendered as |
| --- | --- | --- |
| `BOOL` | 4 | `true` or `false` |
| `LONG`, `ULONG` | 4 | integer |
| `LONG2` | 8 | `a,b` |
| `LONG4` | 16 | `a,b,c,d` |
| `FLOAT` | 4 | `x.xxx` |
| `FLOAT2`, `FLOAT3`, `FLOAT4` | 8, 12, 16 | comma separated `x.xxx` |
| `DOUBLE` | 8 | `x.xxx` |
| `BYTE4` | 4 | `a,b,c,d` |
| `GUID`, `OUTPUTVALUE` | 16 | `{GUID}` |
| `ENUM` | 4 | enum value name |
| `PBH`, `PBH32` | 16 | `pitch,bank,heading` |
| `LLA` | 24 | `Nd m' s",Ed m' s",+alt` |
| `FILETIME` | 8 | not rendered |

Variable length types read an `i32` length first:

| Type | Rendered as |
| --- | --- |
| `TEXT`, `MLTEXT` | decoded string |
| `BEZIERCURVE` | UTF-8 string, trailing nulls trimmed |

A property whose definition marks it as an attribute is attached to the parent
element as an attribute rather than as a child element.

## Float input pins

In the 2024 format a float typed property can be stored as an input pin: a
source GUID followed by one or more floats. The source GUID is the node the pin
is wired to, or the zero GUID when the pin holds a constant.

The stored value size distinguishes the two forms. For a float type with
`components` declared components:

- If the value size equals `components * 4`, the value is a plain float vector
  and is read as that many floats.
- Otherwise the value is an input pin. The first 16 bytes are the source GUID,
  and the remaining bytes are `(size - 16) / 4` floats.

For example a `FLOAT` property with a value size of 20 is a pin: a 16 byte GUID
and one float, rendered as `{GUID},x.xxx`. A `FLOAT4` property with a value size
of 28 is a pin with three floats, rendered as `{GUID},x.xxx,x.xxx,x.xxx`.

## Type coverage

Every value type used by a property in the MSFS 2024 Common propdefs is
supported, including the full input pin family: `INPUTBOOL`, `INPUTLONG`,
`INPUTULONG`, `INPUTVARIANT`, `INPUTFLOAT`, `INPUTFLOAT2`, `INPUTFLOAT3`,
`INPUTCOLOR` and `OUTPUTVALUE`.

Type names are matched case insensitively. The propdefs mix spellings such as
`Float`, `Bool` and `Text` with the canonical upper case forms, and both resolve
to the same type.

`DISABLABLEFLOAT` and `DISABLABLEFLOAT3` are declared in the schema but are not
referenced by any property in the propdefs, so they never appear in a compiled
file and are not implemented.

## Text encoding

`TEXT` and `MLTEXT` values use a position dependent substitution table. The
cipher byte for a character depends on the character and its position in the
string, with the position wrapping every 250 characters. The final byte is a
terminator that decodes to the null character.

The decoding table is shipped as `assets/textdecode.bin`, a sequence of 256
records (one per source character code), each a little endian `u16` length
followed by that many cipher bytes. The table data is derived from
[leppie/spb2xml](https://github.com/leppie/spb2xml) (`TextDecode.Data.cs`).
