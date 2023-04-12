using System.Runtime.InteropServices;

namespace ImageViewer
{
    internal class VirtualKeyboard
    {
        private const int KEYEVENTF_KEYDOWN = 0x0000;        // New definition
        private const int KEYEVENTF_EXTENDEDKEY = 0x0001;    // Key down flag
        private const int KEYEVENTF_KEYUP = 0x0002;          // Key up flag
        private const int VK_LCONTROL = 0xA2;                // Left Control key code

        private static bool _LeftControlKeyPressed;

        [DllImport("user32.dll")]
        private static extern void keybd_event(byte b_vk, byte b_scan, uint dw_flags, int dw_extra_info);

        public static bool ControlPressed()
        {
            return _LeftControlKeyPressed;
        }

        public static void ControlPress()
        {
            keybd_event(VK_LCONTROL, 0x45, KEYEVENTF_EXTENDEDKEY | KEYEVENTF_KEYDOWN, 0);
            _LeftControlKeyPressed = true;
        }

        public static void ControlRelease()
        {
            keybd_event(VK_LCONTROL, 0x45, KEYEVENTF_EXTENDEDKEY | KEYEVENTF_KEYUP, 0);
            _LeftControlKeyPressed = false;
        }
    }
}
