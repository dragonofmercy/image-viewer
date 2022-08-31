﻿using System.Collections.Generic;

namespace ImageViewer.Localization
{
    internal static class En
    {
        public static Dictionary<string, string> GetStrings()
        {
            return new Dictionary<string, string>(){

                { "FILE_TYPE_IMAGE_JPG", "Image JPEG" },
                { "FILE_TYPE_IMAGE_PNG", "Image PNG" },

                { "FILE_INFORMATION_TITLE", "File informations" },
                { "FILE_INFORMATION_DIMENSIONS", "File dimensions" },
                { "FILE_INFORMATION_FOLDER_PATH", "Folder path" },

                { "FOOTER_TOOLBAR_MENU", "Menu" },
                { "FOOTER_TOOLBAR_MENU_FILE_OPEN", "Open image" },
                { "FOOTER_TOOLBAR_MENU_FILE_INFO", "File info" },
                { "FOOTER_TOOLBAR_MENU_FILE_SAVE", "Save as..." },
                { "FOOTER_TOOLBAR_MENU_FILE_DELETE", "Delete image" },
                { "FOOTER_TOOLBAR_MENU_QUIT", "Quit" },

                { "FOOTER_TOOLBAR_IMAGE_ADJUST", "Fit image" },
                { "FOOTER_TOOLBAR_IMAGE_ZOOM_100", "Zoom to 100%" },
                { "FOOTER_TOOLBAR_IMAGE_PREVIOUS", "Previous image" },
                { "FOOTER_TOOLBAR_IMAGE_ZOOM_IN", "Zoom in" },
                { "FOOTER_TOOLBAR_IMAGE_ZOOM_OUT", "Zoom out" },
                { "FOOTER_TOOLBAR_IMAGE_NEXT", "Next image" },

                { "FOOTER_TOOLBAR_TRANSFORM_MENU", "Transform image" },
                { "FOOTER_TOOLBAR_TRANSFORM_ROTATE_LEFT", "Rotate left" },
                { "FOOTER_TOOLBAR_TRANSFORM_ROTATE_RIGHT", "Rotate right" },
                { "FOOTER_TOOLBAR_TRANSFORM_FLIP_HORZ", "Flip horizontal" },
                { "FOOTER_TOOLBAR_TRANSFORM_FLIP_VERT", "Flip vertical" },

            };
        }
    }
}
