using System.Collections.Generic;

namespace ImageViewer.Localization
{
    internal static class En
    {
        public static Dictionary<string, string> GetStrings()
        {
            return new Dictionary<string, string>(){

                { "SYSTEM_PASTED_CONTENT", "Pasted content" },

                { "FILE_TYPE_IMAGE_JPG", "Image JPEG" },
                { "FILE_TYPE_IMAGE_PNG", "Image PNG" },
                { "FILE_TYPE_IMAGE_WEBP", "Image WEBP" },

                { "FILE_INFORMATION_TITLE", "File informations" },
                { "FILE_INFORMATION_DIMENSIONS", "File dimensions" },
                { "FILE_INFORMATION_FOLDER_PATH", "Folder path" },

                { "FOOTER_TOOLBAR_MENU", "Menu" },
                { "FOOTER_TOOLBAR_MENU_FILE_OPEN", "Open image" },
                { "FOOTER_TOOLBAR_MENU_FILE_INFO", "File info" },
                { "FOOTER_TOOLBAR_MENU_FILE_SAVE", "Save as..." },
                { "FOOTER_TOOLBAR_MENU_FILE_DELETE", "Delete image" },
                { "FOOTER_TOOLBAR_MENU_ABOUT", "About" },
                { "FOOTER_TOOLBAR_MENU_QUIT", "Quit" },

                { "FOOTER_TOOLBAR_IMAGE_ADJUST", "Zoom to fit" },
                { "FOOTER_TOOLBAR_IMAGE_ZOOM_100", "Zoom to actual size" },
                { "FOOTER_TOOLBAR_IMAGE_PREVIOUS", "Previous image" },
                { "FOOTER_TOOLBAR_IMAGE_ZOOM_IN", "Zoom in" },
                { "FOOTER_TOOLBAR_IMAGE_ZOOM_OUT", "Zoom out" },
                { "FOOTER_TOOLBAR_IMAGE_NEXT", "Next image" },

                { "FOOTER_TOOLBAR_TRANSFORM_MENU", "Transform image" },
                { "FOOTER_TOOLBAR_TRANSFORM_ROTATE_LEFT", "Rotate left" },
                { "FOOTER_TOOLBAR_TRANSFORM_ROTATE_RIGHT", "Rotate right" },
                { "FOOTER_TOOLBAR_TRANSFORM_FLIP_HORZ", "Flip horizontal" },
                { "FOOTER_TOOLBAR_TRANSFORM_FLIP_VERT", "Flip vertical" },

                { "ABOUT_LINK_GITHUB_REPOSITORY", "GitHub Repository" },
                { "ABOUT_LINK_LATEST_RELEASE", "Latest releases" },
                { "ABOUT_LABEL_LAST_UPDATE", "Last checked: " },
                { "ABOUT_LABEL_LAST_UPDATE_NEVER", "never" },

                { "ABOUT_BTN_CHECK_UPDATE", "Check for update" },
                { "ABOUT_BTN_DOWNLOAD_UPDATE", "Download update" },
                { "ABOUT_BTN_DOWNLOAD_UPDATE_DOWNLOADING", "Downloading..." },

                { "ABOUT_UPDATE_CHECKING", "Checking for update..." },
                { "ABOUT_UPDATE_INFO_UPDATE_LATEST", "Image Viewer is up to date." },
                { "ABOUT_UPDATE_INFO_UPDATE_AVAILABLE", "An update is available." },
                { "ABOUT_UPDATE_INFO_ERROR_NO_INTERNET", "No internet access." },
                { "ABOUT_UPDATE_INFO_ERROR_KEY_NOT_FOUND", "Cannot get last version manifest." },
                
            };
        }
    }
}
