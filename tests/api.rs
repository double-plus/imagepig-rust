use imagepig::ImagePig;
use std::env;
use std::fs;

#[tokio::test]
#[allow(unused_must_use)]
async fn test_api() {
    fs::create_dir_all("output");

    let jane = "https://imagepig.com/static/jane.jpeg";
    let mona_lisa = "https://imagepig.com/static/mona-lisa.jpeg";
    let api_key = env::var("IMAGEPIG_API_KEY").unwrap();

    let imagepig = ImagePig::new(api_key, None);

    imagepig
        .default("pig", None, None)
        .await
        .unwrap()
        .save("output/pig1.jpeg")
        .await;
    imagepig
        .xl("pig", None, None)
        .await
        .unwrap()
        .save("output/pig2.jpeg")
        .await;
    imagepig
        .flux("pig", None, None)
        .await
        .unwrap()
        .save("output/pig3.jpeg")
        .await;
    imagepig
        .faceswap(jane, mona_lisa, None)
        .await
        .unwrap()
        .save("output/faceswap.jpeg")
        .await;
    imagepig
        .upscale(jane, None, None)
        .await
        .unwrap()
        .save("output/upscale.jpeg")
        .await;
    imagepig
        .cutout(jane, None)
        .await
        .unwrap()
        .save("output/cutout.png")
        .await;
    imagepig
        .replace(jane, "woman", "robot", None, None)
        .await
        .unwrap()
        .save("output/replace.jpeg")
        .await;
    imagepig
        .outpaint(jane, "dress", None, None, Some(500), None, None, None)
        .await
        .unwrap()
        .save("output/outpaint.jpeg")
        .await;
}
