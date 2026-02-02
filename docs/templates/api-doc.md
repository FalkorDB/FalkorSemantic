# API Documentation Template

## Module: `module_name`

Brief description of what this module does.

## Types

### `TypeName`

```rust
pub struct TypeName {
    field1: Type1,
    field2: Type2,
}
```

Description of the type and its purpose.

**Fields:**
- `field1`: Description of field1
- `field2`: Description of field2

## Enums

### `EnumName`

```rust
pub enum EnumName {
    Variant1,
    Variant2(Type),
}
```

Description of the enum.

**Variants:**
- `Variant1`: Description
- `Variant2`: Description

## Functions

### `function_name`

```rust
pub fn function_name(param1: Type1, param2: Type2) -> Result<ReturnType>
```

**Description:**
Detailed description of what the function does.

**Parameters:**
- `param1`: Description of param1
- `param2`: Description of param2

**Returns:**
Description of the return value.

**Errors:**
- `ErrorType1`: When this error occurs
- `ErrorType2`: When this error occurs

**Examples:**

```rust
use module_name::function_name;

let result = function_name(arg1, arg2)?;
println!("Result: {:?}", result);
```

**Performance:**
Notes about performance characteristics, if relevant.

**Thread Safety:**
Whether the function is thread-safe and any concurrency considerations.

## Constants

### `CONSTANT_NAME`

```rust
pub const CONSTANT_NAME: Type = value;
```

Description of the constant and its purpose.

## Traits

### `TraitName`

```rust
pub trait TraitName {
    fn method_name(&self) -> ReturnType;
}
```

**Description:**
What the trait represents and when to implement it.

**Required Methods:**
- `method_name`: Description

**Examples:**

```rust
struct MyType;

impl TraitName for MyType {
    fn method_name(&self) -> ReturnType {
        // implementation
    }
}
```

## Usage Examples

### Basic Usage

```rust
// Example showing common use case
```

### Advanced Usage

```rust
// Example showing advanced features
```

## See Also

- Related modules
- Related types
- External documentation
