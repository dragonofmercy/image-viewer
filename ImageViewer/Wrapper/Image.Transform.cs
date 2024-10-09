using SixLabors.ImageSharp;
using SixLabors.ImageSharp.Processing;
using SixLabors.ImageSharp.Processing.Processors.Transforms;

namespace ImageViewer.Wrapper;

internal partial class Image
{
    public void RotateFlip(RotateMode rotateMode, FlipMode flipMode)
    {
        if(!WorkingImageLoaded) return;
        WorkingImage.Mutate(ctx => ctx.RotateFlip(rotateMode, flipMode));
    }

    public void Resize(int width, int height, IResampler mode)
    {
        if(!WorkingImageLoaded) return;
        WorkingImage.Mutate(ctx => ctx.Resize(width, height, mode));
    }

    public void Crop(int x, int y, int cropWidth, int cropHeight)
    {
        if(!WorkingImageLoaded) return;
        WorkingImage.Mutate(ctx => ctx.Crop(new Rectangle(x, y, cropWidth, cropHeight)));
    }
}