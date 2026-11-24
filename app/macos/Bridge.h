// What the app exports to the native interface (app/src/native.rs).

#include <stdbool.h>

/// The file behind a url of the app's UI, or null; freed with arbalete_free_string.
char *arbalete_ui_file(const char *url);

void arbalete_free_string(char *string);

/// The window `id` resized, went into full screen or out of it.
void arbalete_window_resized(unsigned int id);

/// The window `id` came in front, or left it.
void arbalete_window_activated(unsigned int id, bool active);

/// Whether the window `id` may close now: the app may first close its pages.
bool arbalete_window_should_close(unsigned int id);

/// The window `id` closed.
void arbalete_window_closed(unsigned int id);

/// Whether the Little Arbalete window `id` may close now: the app first closes its page, which
/// closes the window.
bool arbalete_little_should_close(unsigned int id);

/// Whether the Picture in Picture window `id` may close now: the app first closes its page, which
/// closes the window.
bool arbalete_picture_should_close(unsigned int id);

/// The Picture in Picture window `id`'s bar asks for its page's tab: it shows, and the window goes.
void arbalete_picture_back(unsigned int id);

/// Runs `script` in the page of the tab `tab`, for AppleScript (Scripting.swift): `done` then takes
/// `context`, the script's result as text, null for none, or why it failed, and whether it ran.
/// False if the tab has no live page, without calling `done`.
bool arbalete_execute_javascript(unsigned long long tab, const char *script,
                                 void (*done)(void *context, const char *result, bool ok),
                                 void *context);
