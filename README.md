# imagepig
[Rust crate](https://crates.io/crates/imagepig) for [Image Pig](https://imagepig.com/), the API for AI images.

## Installation

```
cargo add imagepig
```

## Example of usage

```rust
use imagepig::ImagePig;

// create instance of API (put here your actual API key)
let imagepig = ImagePig::new(api_key, None);

// call the API with a prompt to generate an image
let result = imagepig.xl("cute piglet running on a green garden", None, None).await.unwrap();

// save image to a file
result.save("cute-piglet.jpeg").await;

// or access image data (Vec[u8])
let data = result.data().await?;
```

## Contact us
Something does not work as expected? Feel free to [send us a message](https://imagepig.com/contact/), we are here for you.
