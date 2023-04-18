using System;
using System.IO;
using System.Linq;
using System.Drawing.Imaging;
using System.Threading;

using Windows.Storage.Streams;

using SixLabors.ImageSharp;
using SixLabors.ImageSharp.Processing;
using SixLabors.ImageSharp.Advanced;
using SixLabors.ImageSharp.Formats;
using SixLabors.ImageSharp.Formats.Jpeg;
using SixLabors.ImageSharp.Formats.Png;

using Svg;

using ImageSharpImage = SixLabors.ImageSharp.Image;
using DrawingImage = System.Drawing.Image;


namespace ImageViewer
{
    internal class Image
    {
        private readonly string[] NativeExtensions = { ".jpg", ".jpeg", ".bmp", ".png", ".gif", ".tif", ".webp" };

        public event EventHandler ImageLoaded;

        protected bool WorkingImageLoaded = false;
        protected ImageSharpImage WorkingImage;
        protected IImageEncoder Encoder = new JpegEncoder();
        
        public void Load(string path)
        {
            LoadImageFromPath(path);
        }

        public void Load(IInputStream stream)
        {
            LoadImageFromMemory(stream);
        }

        public bool Loaded
        {
            get
            {
                return WorkingImageLoaded;
            }
        }

        public double Height
        {
            get { return WorkingImage.Height; }
        }

        public double Width
        {
            get { return WorkingImage.Width; }
        }

        public void Dispose()
        {
            WorkingImage?.Dispose();
            WorkingImageLoaded = false;
        }

        public void RotateFlip(RotateMode rotateMode, FlipMode flipMode)
        {
            if(!WorkingImageLoaded) return;
            WorkingImage.Mutate(x => x.RotateFlip(rotateMode, flipMode));
        }

        public string GetImageDimensionsAsString()
        {
            if(!WorkingImageLoaded) return "";
            return WorkingImage.Width + "x" + WorkingImage.Height;
        }

        public string GetDpiAsString()
        {
            if(!WorkingImageLoaded) return "";
            return string.Concat((WorkingImage.Metadata.HorizontalResolution == 1 ? 96 : WorkingImage.Metadata.HorizontalResolution).ToString(), " dpi");
        }

        public string GetDepthAsString()
        {
            if(!WorkingImageLoaded) return "";
            return WorkingImage.PixelType.BitsPerPixel + " bit";
        }

        public IRandomAccessStream GetBitmapImageSource()
        {
            if(WorkingImage == null) return null;

            MemoryStream memory = new();
            WorkingImage.Save(memory, Encoder);
            memory.Position = 0;

            return memory.AsRandomAccessStream();
        }

        public async void Save(string path, string type)
        {
            switch(type)
            {
                case ".jpg":
                    await WorkingImage.SaveAsJpegAsync(path, new JpegEncoder{ Quality = 100 });
                    break;

                case ".png":
                    await WorkingImage.SaveAsPngAsync(path);
                    break;

                case ".webp":
                    await WorkingImage.SaveAsWebpAsync(path);
                    break;
            }
        }

        private async void LoadImageFromPath(string path)
        {
            string extension = Path.GetExtension(path).ToLower();

            if(NativeExtensions.Contains(extension))
            {
                WorkingImage = await ImageSharpImage.LoadAsync(path, CancellationToken.None);
                Encoder = WorkingImage.DetectEncoder(path);
            }
            else
            {
                DrawingImage tmp;

                if(extension == ".svg")
                {
                    SvgDocument svgDocument = SvgDocument.Open(path);
                    svgDocument.ShapeRendering = SvgShapeRendering.Auto;
                    tmp = svgDocument.AdjustSize(1024, 1024).Draw();

                    Encoder = new PngEncoder();
                }
                else
                {
                    byte[] fileBytes = await File.ReadAllBytesAsync(path);
                    using MemoryStream defaultMemoryStream = new(fileBytes);
                    tmp = DrawingImage.FromStream(defaultMemoryStream);
                    defaultMemoryStream.Dispose();
                }

                using MemoryStream saveMemoryStream = new();
                tmp.Save(saveMemoryStream, ImageFormat.Png);
                WorkingImage = ImageSharpImage.Load(saveMemoryStream.ToArray());
                saveMemoryStream.Dispose();
            }

            WorkingImageLoaded = true;
            ImageLoaded?.Invoke(this, EventArgs.Empty);
        }

        private async void LoadImageFromMemory(IInputStream stream)
        {
            WorkingImage = await ImageSharpImage.LoadAsync(stream.AsStreamForRead());

            WorkingImageLoaded = true;
            ImageLoaded?.Invoke(this, EventArgs.Empty);
        }
    }
}
