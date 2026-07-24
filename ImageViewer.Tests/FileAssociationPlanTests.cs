using ImageViewer.Services;
using ImageViewer.Wrapper;

using Xunit;

namespace ImageViewer.Tests;

public class FileAssociationPlanTests
{
    // A path with a space, so the quoting of the shell command is actually exercised.
    private const string EXE = @"C:\Program Files\Image Viewer\ImageViewer.exe";

    private static FileAssociationPlan BuildPlan() => new(EXE, Image.SupportedFileTypes);

    [Theory]
    [InlineData(".jpg", "ImageViewer.jpg")]
    [InlineData(".webp", "ImageViewer.webp")]
    [InlineData(".PNG", "ImageViewer.png")]
    public void ProgIdFor_PrefixesTheLowerCasedExtension(string extension, string expected)
    {
        Assert.Equal(expected, FileAssociationPlan.ProgIdFor(extension));
    }

    [Fact]
    public void ExpectedCommand_QuotesBothTheExeAndTheArgument()
    {
        Assert.Equal("\"" + EXE + "\" \"%1\"", BuildPlan().ExpectedCommand);
    }

    [Fact]
    public void EntriesToWrite_RegistersAnOpenCommandForEverySupportedExtension()
    {
        FileAssociationPlan plan = BuildPlan();
        var entries = plan.EntriesToWrite();

        foreach (string extension in Image.SupportedFileTypes)
        {
            string expectedKey = FileAssociationPlan.ProgIdKeyPath(extension) + @"\shell\open\command";
            Assert.Contains(entries, e => e.KeyPath == expectedKey && e.ValueName == null && e.Value == plan.ExpectedCommand);
        }
    }

    [Fact]
    public void EntriesToWrite_LabelsEachProgIdAndPointsItsIconAtTheExe()
    {
        FileAssociationPlan plan = BuildPlan();
        var entries = plan.EntriesToWrite();

        Assert.Contains(entries, e => e.KeyPath == FileAssociationPlan.ProgIdKeyPath(".jpg") && e.ValueName == null && e.Value == "JPEG image");
        Assert.Contains(entries, e => e.KeyPath == FileAssociationPlan.ProgIdKeyPath(".svg") && e.ValueName == null && e.Value == "SVG image");

        foreach (string extension in Image.SupportedFileTypes)
        {
            string iconKey = FileAssociationPlan.ProgIdKeyPath(extension) + @"\DefaultIcon";
            Assert.Contains(entries, e => e.KeyPath == iconKey && e.ValueName == null && e.Value == EXE + ",0");
        }
    }

    [Fact]
    public void EntriesToWrite_AddsTheProgIdToOpenWithProgidsForEverySupportedExtension()
    {
        var entries = BuildPlan().EntriesToWrite();

        foreach (string extension in Image.SupportedFileTypes)
        {
            string expectedKey = FileAssociationPlan.OpenWithProgidsKeyPath(extension);
            Assert.Contains(entries, e => e.KeyPath == expectedKey && e.ValueName == FileAssociationPlan.ProgIdFor(extension) && e.Value == "");
        }
    }

    // The core guard of the whole design: we only ever propose ourselves. Writing the
    // (Default) value of an extension key would hijack another application's association.
    [Fact]
    public void EntriesToWrite_NeverWritesTheDefaultValueOfAnExtensionKey()
    {
        var entries = BuildPlan().EntriesToWrite();

        foreach (string extension in Image.SupportedFileTypes)
        {
            string extensionKey = FileAssociationPlan.ExtensionKeyPath(extension);
            Assert.DoesNotContain(entries, e => e.KeyPath == extensionKey && string.IsNullOrEmpty(e.ValueName));
        }
    }

    // Adding a format to Image.SupportedFileTypes without a matching TypeLabels entry would
    // silently ship the fallback "Image file" label. Pin completeness the same way CultureTests
    // pins resource-key parity across languages.
    [Fact]
    public void LabelFor_HasARealLabelForEverySupportedExtension()
    {
        foreach (string extension in Image.SupportedFileTypes)
        {
            Assert.NotEqual("Image file", FileAssociationPlan.LabelFor(extension));
        }
    }

    [Fact]
    public void ApplicationKeyPath_UsesTheExecutableFileName()
    {
        Assert.Equal(@"Software\Classes\Applications\ImageViewer.exe", BuildPlan().ApplicationKeyPath);
    }

    [Fact]
    public void EntriesToWrite_DeclaresTheApplicationBlockAndEverySupportedType()
    {
        FileAssociationPlan plan = BuildPlan();
        var entries = plan.EntriesToWrite();

        Assert.Contains(entries, e => e.KeyPath == plan.ApplicationKeyPath && e.ValueName == "FriendlyAppName" && e.Value == "Image Viewer");
        Assert.Contains(entries, e => e.KeyPath == plan.ApplicationKeyPath + @"\shell\open\command" && e.ValueName == null && e.Value == plan.ExpectedCommand);

        foreach (string extension in Image.SupportedFileTypes)
        {
            Assert.Contains(entries, e => e.KeyPath == plan.ApplicationKeyPath + @"\SupportedTypes" && e.ValueName == extension.ToLowerInvariant() && e.Value == "");
        }
    }

    [Fact]
    public void CommandMatchesCurrentExe_IgnoresCaseAndRejectsAnotherExe()
    {
        FileAssociationPlan plan = BuildPlan();

        Assert.True(plan.CommandMatchesCurrentExe(plan.ExpectedCommand.ToUpperInvariant()));
        Assert.False(plan.CommandMatchesCurrentExe(@"""C:\Other\ImageViewer.exe"" ""%1"""));
        Assert.False(plan.CommandMatchesCurrentExe(null));
    }

    [Fact]
    public void Stamp_ChangesWithTheExecutablePath()
    {
        FileAssociationPlan installed = BuildPlan();
        FileAssociationPlan moved = new(@"D:\Portable\ImageViewer.exe", Image.SupportedFileTypes);

        Assert.NotEqual(installed.Stamp, moved.Stamp);
        Assert.Contains(EXE, installed.Stamp);
    }

    [Fact]
    public void Stamp_ChangesWithTheExtensionSet()
    {
        FileAssociationPlan fewer = new(EXE, [".jpg", ".png"]);
        FileAssociationPlan more = new(EXE, [".jpg", ".png", ".webp"]);

        Assert.NotEqual(fewer.Stamp, more.Stamp);
    }
}
