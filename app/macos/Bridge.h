// What the app exports to the native interface (app/src/native.rs).

/// The file behind a url of the app's UI, or null; freed with arbalete_free_string.
char *arbalete_ui_file(const char *url);

void arbalete_free_string(char *string);
