package com.projetotempo;

import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;

/** Botões "Pausar"/"Parar" da notificação e alarme de fim do timer. */
public final class TimerReceiver extends BroadcastReceiver {
    @Override
    public void onReceive(Context ctx, Intent intent) {
        String action = intent.getAction();
        if (action == null) {
            return;
        }
        switch (action) {
            case Bridge.ACTION_PAUSE:
                Native.command(ctx, "pause");
                break;
            case Bridge.ACTION_STOP:
                Native.command(ctx, "stop");
                break;
            case Bridge.ACTION_DEADLINE:
                Bridge.notifyFinished(ctx,
                        intent.getStringExtra(Bridge.EXTRA_TAG),
                        intent.getBooleanExtra(Bridge.EXTRA_SOUND, true),
                        intent.getBooleanExtra(Bridge.EXTRA_VIBRATE, true));
                Native.command(ctx, "deadline");
                break;
            default:
                break;
        }
    }
}
