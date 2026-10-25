// Nested-session fixture only: its output file contains synthetic test text.
#include <gtk/gtk.h>
#include <unistd.h>

static GtkWidget *prose, *password, *main_window, *screenshot_window;
static GtkApplication *screenshot_app;
static const char *output;

static void changed(GtkTextBuffer *buffer, gpointer unused) {
    (void)unused;
    GtkTextIter start, end;
    gtk_text_buffer_get_bounds(buffer, &start, &end);
    char *text = gtk_text_buffer_get_text(buffer, &start, &end, FALSE);
    g_file_set_contents(output, text, -1, NULL);
    g_free(text);
}

static gboolean command(GIOChannel *channel, GIOCondition condition, gpointer unused) {
    (void)unused;
    if (condition & (G_IO_HUP | G_IO_ERR)) return G_SOURCE_REMOVE;
    char *line = NULL;
    if (g_io_channel_read_line(channel, &line, NULL, NULL, NULL) == G_IO_STATUS_NORMAL) {
        if (g_str_has_prefix(line, "password")) gtk_widget_grab_focus(password);
        else if (g_str_has_prefix(line, "prose")) {
            if (screenshot_window) {
                gtk_window_destroy(GTK_WINDOW(screenshot_window));
                screenshot_window = NULL;
            }
            gtk_window_present(GTK_WINDOW(main_window));
            gtk_widget_grab_focus(prose);
        }
        else if (g_str_has_prefix(line, "screenshot")) {
            if (!screenshot_app) {
                screenshot_app = gtk_application_new("org.flameshot.Flameshot", G_APPLICATION_NON_UNIQUE);
                g_application_register(G_APPLICATION(screenshot_app), NULL, NULL);
            }
            screenshot_window = gtk_application_window_new(screenshot_app);
            gtk_window_set_title(GTK_WINDOW(screenshot_window), "flameshot");
            gtk_window_set_child(GTK_WINDOW(screenshot_window), gtk_label_new("Synthetic screenshot overlay"));
            gtk_window_fullscreen(GTK_WINDOW(screenshot_window));
            gtk_window_present(GTK_WINDOW(screenshot_window));
        }
        else if (g_str_has_prefix(line, "clear")) gtk_text_buffer_set_text(gtk_text_view_get_buffer(GTK_TEXT_VIEW(prose)), "", 0);
    }
    g_free(line);
    return G_SOURCE_CONTINUE;
}

static void activate(GtkApplication *application, gpointer unused) {
    (void)unused;
    GtkWidget *window = gtk_application_window_new(application);
    main_window = window;
    gtk_window_set_title(GTK_WINDOW(window), "Writy synthetic desktop fixture");
    gtk_window_set_default_size(GTK_WINDOW(window), 650, 420);
    GtkWidget *box = gtk_box_new(GTK_ORIENTATION_VERTICAL, 12);
    prose = gtk_text_view_new();
    gtk_text_view_set_wrap_mode(GTK_TEXT_VIEW(prose), GTK_WRAP_WORD_CHAR);
    gtk_widget_set_vexpand(prose, TRUE);
    password = gtk_password_entry_new();
    gtk_box_append(GTK_BOX(box), gtk_label_new("Synthetic prose; fixture contents written only to the test directory"));
    gtk_box_append(GTK_BOX(box), prose);
    gtk_box_append(GTK_BOX(box), gtk_label_new("Password fixture: input is never written to the output file"));
    gtk_box_append(GTK_BOX(box), password);
    gtk_window_set_child(GTK_WINDOW(window), box);
    g_signal_connect(gtk_text_view_get_buffer(GTK_TEXT_VIEW(prose)), "changed", G_CALLBACK(changed), NULL);
    GIOChannel *input = g_io_channel_unix_new(STDIN_FILENO);
    g_io_add_watch(input, G_IO_IN | G_IO_HUP | G_IO_ERR, command, NULL);
    g_io_channel_unref(input);
    gtk_window_present(GTK_WINDOW(window));
    gtk_widget_grab_focus(prose);
}

int main(int argc, char **argv) {
    if (argc != 2) return 2;
    output = argv[1];
    GtkApplication *application = gtk_application_new("org.example.WritingFixture", G_APPLICATION_NON_UNIQUE);
    g_signal_connect(application, "activate", G_CALLBACK(activate), NULL);
    int status = g_application_run(G_APPLICATION(application), 1, argv);
    g_object_unref(application);
    return status;
}
