﻿using System.Collections.Generic;

namespace ImageViewer.Strings;

internal static class En
{
    public static Dictionary<string, string> GetStrings()
    {
        return new Dictionary<string, string>(){

            { "DEFAULT_SYSTEM_LANGUAGE", "System language" },
            { "SYSTEM_PASTED_CONTENT", "Pasted content" },
            { "SYSTEM_LOADING_ERROR", "Sorry, the application cannot open this file because the format is currently not supported or the file is damaged" },
            { "SYSTEM_LOADING", "Loading" },

            { "SETTINGS_FIELD_LANGUAGE", "Language" },
            { "SETTINGS_FIELD_LANGUAGE_HELP", "You need to restart the application to apply this setting." },
            { "SETTINGS_FIELD_UPDATE_INTERVAL", "Find updates" },
            { "SETTINGS_FIELD_UPDATE_INTERVAL_DAY", "Every days" },
            { "SETTINGS_FIELD_UPDATE_INTERVAL_WEEK", "Every weeks" },
            { "SETTINGS_FIELD_UPDATE_INTERVAL_MONTH", "Every months" },
            { "SETTINGS_FIELD_UPDATE_INTERVAL_MANUAL", "Manually" },

            { "FILE_INFORMATION_TITLE", "File informations" },
            { "FILE_INFORMATION_DIMENSIONS", "File dimensions" },
            { "FILE_INFORMATION_FOLDER_PATH", "Folder path" },
            { "FILE_INFORMATION_FOLDER_PATH_TIP", "Display directory in Windows Explorer" },

            { "FOOTER_TOOLBAR_MENU", "Menu" },
            { "FOOTER_TOOLBAR_MENU_FILE_OPEN", "Open image" },
            { "FOOTER_TOOLBAR_MENU_FILE_INFO", "File info" },
            { "FOOTER_TOOLBAR_MENU_FILE_SAVE", "Save as..." },
            { "FOOTER_TOOLBAR_MENU_FILE_SAVE_FORMAT", "{0} image" },
            { "FOOTER_TOOLBAR_MENU_FILE_DELETE", "Delete image" },
            { "FOOTER_TOOLBAR_MENU_ABOUT", "About" },
            { "FOOTER_TOOLBAR_MENU_OPTIONS", "Settings" },
            { "FOOTER_TOOLBAR_MENU_QUIT", "Quit" },

            { "FOOTER_TOOLBAR_IMAGE_ADJUST", "Zoom to fit" },
            { "FOOTER_TOOLBAR_IMAGE_ZOOM_100", "Zoom to actual size" },
            { "FOOTER_TOOLBAR_IMAGE_PREVIOUS", "Previous image" },
            { "FOOTER_TOOLBAR_IMAGE_ZOOM_IN", "Zoom in" },
            { "FOOTER_TOOLBAR_IMAGE_ZOOM_OUT", "Zoom out" },
            { "FOOTER_TOOLBAR_IMAGE_NEXT", "Next image" },

            { "FOOTER_TOOLBAR_TRANSFORM_MENU", "Transform image" },
            { "FOOTER_TOOLBAR_TRANSFORM_CROP", "Crop image" },
            { "FOOTER_TOOLBAR_TRANSFORM_ROTATE_LEFT", "Rotate left" },
            { "FOOTER_TOOLBAR_TRANSFORM_ROTATE_RIGHT", "Rotate right" },
            { "FOOTER_TOOLBAR_TRANSFORM_FLIP_HORZ", "Flip horizontal" },
            { "FOOTER_TOOLBAR_TRANSFORM_FLIP_VERT", "Flip vertical" },

            { "TRANSFORM_CROP_TITLE", "Crop tool" },
            { "TRANSFORM_CROP_RATIO", "Ratio" },
            { "TRANSFORM_CROP_FREE", "No constraints" },
            { "TRANSFORM_CROP_SAME", "Original proportions" },
            { "TRANSFORM_CROP_RESET", "Reset" },
            { "TRANSFORM_CROP_VALIDATE", "Validate" },

            { "ABOUT_LINK_GITHUB_REPOSITORY", "GitHub Repository" },
            { "ABOUT_LINK_LATEST_RELEASE", "Latest releases" },
            { "ABOUT_LABEL_LAST_UPDATE", "Last checked: " },
            { "ABOUT_LABEL_LAST_UPDATE_NEVER", "never" },

            { "ABOUT_BTN_CHECK_UPDATE", "Check for update" },
            { "ABOUT_BTN_DOWNLOAD_UPDATE", "Download update" },
            { "ABOUT_BTN_DOWNLOAD_UPDATE_DOWNLOADING", "Downloading..." },
            { "ABOUT_BTN_DOWNLOAD_UPDATE_RETRY", "Retry" },

            { "ABOUT_UPDATE_CHECKING", "Checking for update..." },
            { "ABOUT_UPDATE_INFO_UPDATE_LATEST", "Image Viewer is up to date." },
            { "ABOUT_UPDATE_INFO_UPDATE_AVAILABLE", "An update is available." },
            { "ABOUT_UPDATE_INFO_ERROR_NO_INTERNET", "No internet access." },
            { "ABOUT_UPDATE_INFO_ERROR_KEY_NOT_FOUND", "Cannot get last version manifest." },

        };
    }
}