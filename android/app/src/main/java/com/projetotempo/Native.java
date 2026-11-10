package com.projetotempo;

import android.content.Context;
import android.util.Log;

/** Ponte Java → Rust para os receptores de broadcast. */
final class Native {
    private static boolean loaded;

    private Native() {
    }

    private static native void onCommand(Context ctx, String filesDir, String command);

    private static synchronized boolean load() {
        if (!loaded) {
            try {
                System.loadLibrary("tempo_app");
                loaded = true;
            } catch (Throwable t) {
                Log.e(Bridge.TAG, "não foi possível carregar libtempo_app.so", t);
            }
        }
        return loaded;
    }

    /** Comandos: "pause", "stop", "deadline", "sync". A regra fica toda no Rust. */
    static void command(Context ctx, String command) {
        if (load()) {
            Context app = ctx.getApplicationContext();
            onCommand(app, app.getFilesDir().getAbsolutePath(), command);
        }
    }
}
