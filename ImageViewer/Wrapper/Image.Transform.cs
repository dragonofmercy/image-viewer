using SixLabors.ImageSharp.Processing;
using SixLabors.ImageSharp.Processing.Processors.Transforms;

namespace ImageViewer.Wrapper;

internal partial class Image
{
    public void RotateFlip(RotateMode rotateMode, FlipMode flipMode)
    {
        if(!WorkingImageLoaded) return;
        WorkingImage.Mutate(x => x.RotateFlip(rotateMode, flipMode));
    }

    public void Resize(int width, int height, IResampler mode)
    {
        if(!WorkingImageLoaded) return;
        WorkingImage.Mutate(x => x.Resize(width, height, mode));
    }
}