using System;

namespace ImageViewer.Utilities;

/// <summary>
/// Small value-formatting and rounding helpers.
/// </summary>
internal static class Format
{
    /// <summary>
    /// Round a value to a multiple of ten (rounds to the nearest integer first, then floors to the ten).
    /// </summary>
    public static float RoundToTen(double input)
    {
        return (int)(Math.Round(input) / 10.0) * 10;
    }

    /// <summary>
    /// Format a byte count as a human readable string (B, KB, MB, GB, TB).
    /// </summary>
    public static string HumanizeBytes(double bytes)
    {
        int order = 0;
        string[] sizes = { "B", "KB", "MB", "GB", "TB" };

        while (bytes >= 1024 && order < sizes.Length - 1)
        {
            order++;
            bytes /= 1024;
        }

        return $"{bytes:0.#} {sizes[order]}";
    }
}
