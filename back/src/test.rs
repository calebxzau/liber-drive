#[cfg(test)]
mod test {

    use std::{
        fs::{self},
        path::{Path, PathBuf},
        str::FromStr,
    };
    use std::fs::File;
    use std::io::BufReader;
    use image::DynamicImage;
    use image::io::Reader;
    use tracing_test::traced_test;

    use crate::{
        media_item::{compress_image_webp, MediaItem},
        util::ResultAnyErr,
    };
    use crate::media_item::{decode_still_image_with_ffmpeg, get_video_first_frame};
      #[test]
        fn read_and_decode_heic_file() -> ResultAnyErr<()> {
            let file_path = "test/image2.heic";
            let img = decode_still_image_with_ffmpeg(Path::new(file_path))?;
            let webp_mem = compress_image_webp(&img, false)?.to_vec();
            fs::write("1.webp", &webp_mem)?;
    
            Ok(())
        }
   /* #[traced_test]
    #[test]
    fn read_and_decode_avif() -> ResultAnyErr<()> {
        let path = Path::new("C:\\Users\\liberatorch\\Downloads\\hato.profile0.10bpc.yuv420.no-cdef.no-restoration.avif");
       // let path = Path::new("E:\\camera\\situMT24\\0903\\Untitled Export\\20240903-_1049864.avif");
        //let image = MediaItem::decode_avif_image(path)?;/*
        let webp_mem = compress_image_webp(&image, false)?.to_vec();
        fs::write("1.webp", &webp_mem)?;*/

        Ok(())
    }*/
    fn _tbnl_maker(tbnl: bool)-> ResultAnyErr<()>{
        let image = decode_still_image_with_ffmpeg(Path::new("test/image2.heic"))?;
        let webp_mem = compress_image_webp(&image, tbnl)?.to_vec();
        fs::write(format!("test/output/image2_{}.webp",tbnl), &webp_mem)?;

        let mut img_reader = Reader::new(BufReader::new(File::open("test/image1.jpg")?)).with_guessed_format()?;
        img_reader.no_limits();
        let image = img_reader.decode()?;
        let webp_mem = compress_image_webp(&image, tbnl)?.to_vec();
        fs::write(format!("test/output/image1_{}.webp",tbnl), &webp_mem)?;

        let image = get_video_first_frame("test/video-h.mp4".to_string())?;
        let webp_mem = compress_image_webp(&image, tbnl)?.to_vec();
        fs::write(format!("test/output/image3_{}.webp",tbnl), &webp_mem)?;
        
        let image = get_video_first_frame("test/video-v.mp4".to_string())?;
        let webp_mem = compress_image_webp(&image, tbnl)?.to_vec();
        fs::write(format!("test/output/image4_{}.webp",tbnl), &webp_mem)?;
        Ok(())
    }
    #[test]
    fn tbnl_maker() -> ResultAnyErr<()>{
        _tbnl_maker(true)?;
        _tbnl_maker(false)?;
        Ok(())
        
    }
}
