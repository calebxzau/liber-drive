use std::ffi::OsStr;
 
use image::imageops::FilterType;
use image::io::Reader;
use image::{DynamicImage, ImageBuffer, Rgb};
use std::fs::File;
use std::io::{BufReader, Cursor, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::{fs, path::PathBuf, time::UNIX_EPOCH};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use log::{debug, info};
use serde::{Deserialize, Serialize};
use webp::WebPMemory;
use crate::album::Album;
use crate::image_exif::ImageExif;
use crate::media_item;
use crate::util::{date_str_to_timestamp, filename_to_timestamp, AnyError, ResultAnyErr};

//图片/视频
#[serde_with::skip_serializing_none]
#[derive(Serialize,Clone)]
pub struct MediaItem { 
    pub name: String,
    #[serde(skip_serializing)]
    pub path: PathBuf,
    pub time: u64,
    pub size: u64,
    pub exif: Option<ImageExif>,
    pub duration: Option<u16>,
    pub pwd: Option<String>
}
#[derive(Serialize,Clone)]
pub struct MediaItemInfo {
    pub name: String,
    pub time: u64,
    pub size: u64,
    pub exif: Option<ImageExif>,
    pub duration: Option<u16>,
}
impl MediaItem {
    pub fn new( path: PathBuf, name: String, time: u64, size: u64) -> Self {
        Self { 
            path,
            name,
            time,
            size,
            exif: None,
            duration: None,
            pwd: None
        }
    }
    //获取扩展名（小写）
    pub fn get_extension(&self) -> String {
        self.path
            .extension()
            .unwrap_or(OsStr::new(""))
            .to_ascii_lowercase()
            .to_string_lossy()
            .into_owned()
    }
    //预览路径
    pub fn get_preview_path(&self, thumbnail: bool) -> PathBuf {
        Path::new("cache").join(format!(
            "tb_{}_{}_{}.webp",
            if thumbnail { 2 } else { 0 },
            self.size,
            self.name.to_lowercase()
        ))
    }
    //是否已创建预览 小图+大图
    pub fn is_preview_created(&self, thumbnail: bool) -> bool {
        self.get_preview_path(thumbnail).exists()
    }
    //获取预览
    pub fn get_preview(&self, thumbnail: bool) -> ResultAnyErr<Vec<u8>> {
        //没预览
        if !self.is_preview_created(thumbnail) {
            //返回空的
            return Err("缩略图不存在".into());
        }
        let mut file = std::fs::File::open(&self.get_preview_path(thumbnail))?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        Ok(buffer)
    }
    //创建预览
    pub fn create_preview(&self, thumbnail: bool) -> ResultAnyErr<()> {
        if self.is_preview_created(thumbnail) {
            debug!("图片{:?}已有缩略图", self.path);
            return Ok(());
        }
        let path = &self.path;
        info!("正在创建缩略图 {:?}", self.name);

        let image = if is_video(path) {
            //是视频 截取第一帧
            get_video_first_frame( path.display().to_string())?
        } else if is_image(path) {
            //是图片
            let file = File::open(path)?;
            let reader = BufReader::new(file); //::open(path)?
            if is_jpg_image(path) {
                let mut img_reader = Reader::new(reader).with_guessed_format()?;
                img_reader.no_limits();
                img_reader.decode()?
            } else if is_heif_image(path) || is_avif_image(path) {
                media_item::decode_still_image_with_ffmpeg(path)?
            } else {
                return Err(format!("{:?} 格式不支持解码", path).into());
            }
        } else {
            return Err(format!("{:?}不是照片视频", path).into());
        };

        //缩略图
        let webp_mem = compress_image_webp(&image, thumbnail)?.to_vec();
        //保存 缓存
        fs::write(&self.get_preview_path(thumbnail), &webp_mem)?;
        info!(
            "已创建{}图，大小{}",
            if thumbnail { "小" } else { "大" },
            webp_mem.len()
        );
        Ok(())
    }


    //exif信息缓存路径
    pub fn get_exif_cache_path(&self) -> PathBuf {
        Path::new("cache").join("exif").join(format!(
            "exif_{}_{}.json",
            self.size,
            self.name.to_lowercase()
        ))
    }
    //视频时长信息缓存路径
    pub fn get_video_duration_cache_path(&self) -> PathBuf {
        Path::new("cache").join("video").join(format!(
            "duration_{}_{}.txt",
            self.size,
            self.name.to_lowercase()
        ))
    }
    //创建exif信息缓存
    pub fn create_exif_cache(&self) -> ResultAnyErr<()> {
        if self.get_exif_cache_path().exists() {
            debug!("缓存已存在，不需要创建");
            return Ok(());
        }
        info!("正在创建exif缓存 {:?}", self.path);
        let exif = ImageExif::from_media_path(&self.path);
        if let Ok(exif) = exif {
            let json = serde_json::to_string(&exif)?;
            fs::write(&self.get_exif_cache_path(), json)?;
        }
        Ok(())
    }
    //读取exif信息缓存
    pub fn read_exif_cache(&mut self) -> ResultAnyErr<()> {
        let path = self.get_exif_cache_path();
        if !path.exists() {
            info!("{:?} 缓存不存在", path);
            return Ok(());
        }
        let data = fs::read_to_string(path)?;
        let exif: ImageExif = serde_json::from_str(&data)?;
        self.exif = Some(exif);
        Ok(())
    }
    //读取视频时长信息缓存
    pub fn read_video_duration_cache(&mut self) -> ResultAnyErr<()> {
        let path = self.get_video_duration_cache_path();
        if !path.exists() {
            info!("{:?} 缓存不存在", path);
            return Ok(());
        }
        let data = fs::read_to_string(path)?.parse::<u16>()?;
        self.duration = Some(data);
        Ok(())
    }
    //创建视频时长信息缓存
    pub fn create_video_duration_cache(&self) -> ResultAnyErr<()> {
        if self.get_video_duration_cache_path().exists() {
            debug!("缓存已存在，不需要创建");
            return Ok(());
        }
        info!("正在创建视频时长缓存 {:?}", self.path);
        let duration = if is_video(&self.path) {
            get_video_duration(self.path.to_str().unwrap())?
        } else {
            return Err(format!("{}不是视频 无法获取视频时长", self.name).into());
        };
        fs::write(&self.get_video_duration_cache_path(), duration.to_string())?;
        Ok(())
    }
    //更新照片修改时间（从exif/文件名读取）
    pub fn update_media_time(&mut self) {
        let mut time = self.time;
        if let Some(exif) = &self.exif {
            //exif里面成功取时间了 就用exif的
            if let Ok(_time) = date_str_to_timestamp(&exif.shot_time) {
                time = _time;
            } else if let Ok(_time) = filename_to_timestamp(&self.name) {
                //exif里没有 就去文件名里取
                time = _time;
            }
            //再取不到 就用系统的修改时间（什么都不动）
        }
        self.time = time;
    }

    pub fn get_media_id(&self) -> String {
        format!("{}_{}",self.size,
                self.name)
    }
}
//运行ffmpeg 输出一帧图片到stdout 返回图片字节
//用output()而不是spawn() 保证子进程被wait回收 不会留下zombie
fn run_ffmpeg_single_frame(input: &OsStr, extra_args: &[&str]) -> ResultAnyErr<Vec<u8>> {
    let output = Command::new("ffmpeg")
        .arg("-nostdin")
        .arg("-v")
        .arg("error")
        .arg("-i")
        .arg(input)
        .arg("-frames:v")
        .arg("1")
        .arg("-f")
        .arg("image2pipe")
        .args(extra_args)
        .arg("-")
        .stdin(Stdio::null())
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "ffmpeg failed ({}) for {:?}: {}",
            output.status,
            input,
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    Ok(output.stdout)
}
pub fn decode_still_image_with_ffmpeg(path: &Path) -> ResultAnyErr<DynamicImage> {
    let buffer = run_ffmpeg_single_frame(path.as_os_str(), &["-vcodec", "png"])?;
    let cursor = Cursor::new(buffer);
    let mut img_reader = Reader::new(cursor).with_guessed_format()?;
    img_reader.no_limits();
    Ok(img_reader.decode()?)
}
//压缩图片为webp格式
pub fn compress_image_webp(image: &DynamicImage, thumbnail: bool) -> ResultAnyErr<WebPMemory> {
    let (width, height) = (image.width(), image.height());
    let (new_width, new_height) = if thumbnail {
        //长边256
        if width > height {
            (256, (256.0 * height as f32 / width as f32).round() as u32)
        } else {
            ((256.0 * width as f32 / height as f32).round() as u32, 256)
        }
    } else {
        //长边2560
        if width > height {
            (2560, (2560.0 * height as f32 / width as f32).round() as u32)
        } else {
            ((2560.0 * width as f32 / height as f32).round() as u32, 2560)
        }
    };

    let resized_image = image.resize(new_width, new_height, FilterType::Nearest);
    let quality = if thumbnail { 60f32 } else { 80f32 };
    let webp_mem = webp::Encoder::from_image(&resized_image)?.encode(quality);

    Ok(webp_mem)
}

//匹配文件扩展名 是否在extensions里
fn matches_extension(path: &PathBuf, extensions: &[&str]) -> bool {
    if let Some(ext) = path.extension() {
        let ext = ext.to_ascii_lowercase();
        let ext = &ext.to_string_lossy();
        
        extensions.iter().any(|&e| ext == e)
    } else {
        false
    }
}
//是否视频
pub fn is_video(path: &PathBuf) -> bool {
    matches_extension(path, &["mp4","mov"])
}
//是否图片
pub fn is_image(path: &PathBuf) -> bool {
    is_jpg_image(path) || is_heif_image(path) || is_avif_image(path)
}
pub fn is_jpg_image(path: &PathBuf) -> bool {
    matches_extension(path, &["jpg", "jpeg"])
}
pub fn is_avif_image(path: &PathBuf) -> bool {
    matches_extension(path, &["avif"])
}
pub fn is_heif_image(path: &PathBuf) -> bool {
    matches_extension(path, &["heif", "heic"])
}
//文件创建时间timestamp
pub fn get_file_created_time(path: &PathBuf) -> std::result::Result<u64, AnyError> {
    let meta = fs::metadata(&path)?;
    let system_time = match meta.created() {
        Ok(time) => time,
        Err(e) if e.kind() == std::io::ErrorKind::Unsupported => meta.modified()?,
        Err(e) => return Err(e.into()),
    };
    let time = system_time.duration_since(UNIX_EPOCH)?.as_secs();
    Ok(time)
}
pub fn get_file_size(path: &PathBuf) -> ResultAnyErr<u64> {
    let meta = fs::metadata(&path)?;
    Ok(meta.len())
}
//读取视频第一帧
pub fn get_video_first_frame(video_path: String) -> ResultAnyErr<DynamicImage> {
    let buffer = run_ffmpeg_single_frame(OsStr::new(&video_path), &[])?;
    let cursor = Cursor::new(buffer);

    let img = image::io::Reader::new(cursor)
        .with_guessed_format()?
        .decode()?;
    Ok(img)
}
//读取视频时长
pub fn get_video_duration(path: &str) -> ResultAnyErr<u16> {
    let output = Command::new("ffprobe")
        .args(&[
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
            path,
        ])
        .output()?;

    let duration_str = std::str::from_utf8(&output.stdout)?.trim();
    let duration = duration_str.parse::<f64>()?;
    Ok(duration as u16)
    /* let file = File::open(path)?;
    let size = file.metadata()?.len();
    let reader = BufReader::new(file);
    let mp4 = Mp4Reader::read_header(reader, size)?;

    Ok(mp4.duration().as_secs() as u16) */
}
