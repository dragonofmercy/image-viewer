using SixLabors.ImageSharp;
using SixLabors.ImageSharp.Processing;

namespace ImageViewer.Wrapper;

internal partial class Image
{
    public void RotateFlip(RotateMode rotateMode, FlipMode flipMode)
    {
        if(!WorkingImageLoaded) return;
        WorkingImage.Mutate(ctx => ctx.RotateFlip(rotateMode, flipMode));
        Modified = true;
    }

    public void Crop(int x, int y, int cropWidth, int cropHeight)
    {
        if(!WorkingImageLoaded) return;
        WorkingImage.Mutate(ctx => ctx.Crop(new Rectangle(x, y, cropWidth, cropHeight)));
        Modified = true;
    }
}