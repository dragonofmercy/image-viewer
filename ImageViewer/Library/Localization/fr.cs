using System.Collections.Generic;

namespace ImageViewer.Localization
{
    internal static class Fr
    {
        public static Dictionary<string, string> GetStrings()
        {
            return new Dictionary<string, string>(){

                { "DEFAULT_SYSTEM_LANGUAGE", "Langue du système" },
                { "SYSTEM_PASTED_CONTENT", "Contenu collé" },

                { "SETTINGS_FIELD_LANGUAGE", "Langue" },

                { "FILE_TYPE_IMAGE_JPG", "Image JPEG" },
                { "FILE_TYPE_IMAGE_PNG", "Image PNG" },
                { "FILE_TYPE_IMAGE_WEBP", "Image WEBP" },

                { "FILE_INFORMATION_TITLE", "Informations de l'image" },
                { "FILE_INFORMATION_DIMENSIONS", "Dimension de l'image" },
                { "FILE_INFORMATION_FOLDER_PATH", "Chemin du dossier" },

                { "FOOTER_TOOLBAR_MENU", "Menu" },
                { "FOOTER_TOOLBAR_MENU_FILE_OPEN", "Ouvrir une image" },
                { "FOOTER_TOOLBAR_MENU_FILE_INFO", "Informations de l'image" },
                { "FOOTER_TOOLBAR_MENU_FILE_SAVE", "Enregistrer sous..." },
                { "FOOTER_TOOLBAR_MENU_FILE_DELETE", "Supprimer l'image" },
                { "FOOTER_TOOLBAR_MENU_ABOUT", "A propos" },
                { "FOOTER_TOOLBAR_MENU_OPTIONS", "Options" },
                { "FOOTER_TOOLBAR_MENU_QUIT", "Quitter" },

                { "FOOTER_TOOLBAR_IMAGE_ADJUST", "Faire un zoom pour ajuster" },
                { "FOOTER_TOOLBAR_IMAGE_ZOOM_100", "Zoom en taille réelle" },
                { "FOOTER_TOOLBAR_IMAGE_PREVIOUS", "Image précédente" },
                { "FOOTER_TOOLBAR_IMAGE_ZOOM_IN", "Zoom avant" },
                { "FOOTER_TOOLBAR_IMAGE_ZOOM_OUT", "Zoom arrière" },
                { "FOOTER_TOOLBAR_IMAGE_NEXT", "Image suivante" },

                { "FOOTER_TOOLBAR_TRANSFORM_MENU", "Transformation de l'image" },
                { "FOOTER_TOOLBAR_TRANSFORM_ROTATE_LEFT", "Rotation gauche" },
                { "FOOTER_TOOLBAR_TRANSFORM_ROTATE_RIGHT", "Rotation droite" },
                { "FOOTER_TOOLBAR_TRANSFORM_FLIP_HORZ", "Mirroir horizontal" },
                { "FOOTER_TOOLBAR_TRANSFORM_FLIP_VERT", "Mirroir vertical" },

                { "ABOUT_LINK_GITHUB_REPOSITORY", "Dépôt GitHub" },
                { "ABOUT_LINK_LATEST_RELEASE", "Dernières versions" },
                { "ABOUT_LABEL_LAST_UPDATE", "Dernière vérification : " },
                { "ABOUT_LABEL_LAST_UPDATE_NEVER", "jamais" },

                { "ABOUT_BTN_CHECK_UPDATE", "Vérifier les mises à jour" },
                { "ABOUT_BTN_DOWNLOAD_UPDATE", "Télecharger la mise à jour" },
                { "ABOUT_BTN_DOWNLOAD_UPDATE_DOWNLOADING", "Télechargement en cours..." },

                { "ABOUT_UPDATE_CHECKING", "Recherche en cours..." },
                { "ABOUT_UPDATE_INFO_UPDATE_LATEST", "Image Viewer est à jour." },
                { "ABOUT_UPDATE_INFO_UPDATE_AVAILABLE", "Une mise à jour est disponible." },
                { "ABOUT_UPDATE_INFO_ERROR_NO_INTERNET", "Pas d'accès à internet." },
                { "ABOUT_UPDATE_INFO_ERROR_KEY_NOT_FOUND", "Impossible de récupèrer la dernière version." },

            };
        }
    }
}
