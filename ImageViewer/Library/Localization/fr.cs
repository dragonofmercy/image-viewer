using System.Collections.Generic;

namespace ImageViewer.Localization
{
    internal static class Fr
    {
        public static Dictionary<string, string> GetStrings()
        {
            return new Dictionary<string, string>(){

                { "FILE_TYPE_IMAGE_JPG", "Image JPEG" },
                { "FILE_TYPE_IMAGE_PNG", "Image PNG" },

                { "FILE_INFORMATION_TITLE", "Informations de l'image" },
                { "FILE_INFORMATION_DIMENSIONS", "Dimension de l'image" },
                { "FILE_INFORMATION_FOLDER_PATH", "Chemin du dossier" },

                { "FOOTER_TOOLBAR_MENU", "Menu" },
                { "FOOTER_TOOLBAR_MENU_FILE_OPEN", "Ouvrir une image" },
                { "FOOTER_TOOLBAR_MENU_FILE_INFO", "Informations de l'image" },
                { "FOOTER_TOOLBAR_MENU_FILE_SAVE", "Enregistrer sous..." },
                { "FOOTER_TOOLBAR_MENU_FILE_DELETE", "Supprimer l'image" },
                { "FOOTER_TOOLBAR_MENU_QUIT", "Quitter" },

                { "FOOTER_TOOLBAR_IMAGE_ADJUST", "Ajuster l'image" },
                { "FOOTER_TOOLBAR_IMAGE_ZOOM_100", "Zoomer à 100%" },
                { "FOOTER_TOOLBAR_IMAGE_PREVIOUS", "Image précédente" },
                { "FOOTER_TOOLBAR_IMAGE_ZOOM_IN", "Zoomer" },
                { "FOOTER_TOOLBAR_IMAGE_ZOOM_OUT", "Dézoomer" },
                { "FOOTER_TOOLBAR_IMAGE_NEXT", "Image suivante" },

                { "FOOTER_TOOLBAR_TRANSFORM_MENU", "Transformation de l'image" },
                { "FOOTER_TOOLBAR_TRANSFORM_ROTATE_LEFT", "Rotation gauche" },
                { "FOOTER_TOOLBAR_TRANSFORM_ROTATE_RIGHT", "Rotation droite" },
                { "FOOTER_TOOLBAR_TRANSFORM_FLIP_HORZ", "Mirroir horizontal" },
                { "FOOTER_TOOLBAR_TRANSFORM_FLIP_VERT", "Mirroir vertical" },

            };
        }
    }
}
