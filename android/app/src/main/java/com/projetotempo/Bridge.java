package com.projetotempo;

import android.Manifest;
import android.app.Activity;
import android.app.AlarmManager;
import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.content.Context;
import android.content.Intent;
import android.content.SharedPreferences;
import android.content.pm.PackageManager;
import android.graphics.drawable.Icon;
import android.media.AudioAttributes;
import android.media.Ringtone;
import android.media.RingtoneManager;
import android.net.Uri;
import android.os.Build;
import android.os.Environment;
import android.os.VibrationEffect;
import android.os.Vibrator;
import android.provider.Settings;
import android.text.format.DateFormat;
import android.util.Log;
import android.widget.Toast;

import java.io.File;
import java.io.FileOutputStream;
import java.nio.charset.StandardCharsets;
import java.util.Date;

/**
 * Camada mínima sobre APIs do Android, chamada pelo Rust via JNI (RNF03).
 * Não contém regra de negócio: recebe valores prontos e só fala com o sistema.
 */
public final class Bridge {
    static final String TAG = "tempo";
    static final String ACTION_PAUSE = "com.projetotempo.PAUSE";
    static final String ACTION_STOP = "com.projetotempo.STOP";
    static final String ACTION_DEADLINE = "com.projetotempo.DEADLINE";
    static final String EXTRA_TAG = "tag";
    static final String EXTRA_SOUND = "sound";
    static final String EXTRA_VIBRATE = "vibrate";

    private static final String CHANNEL_TIMER = "timer";
    private static final String CHANNEL_DONE = "fim";
    private static final int ID_TIMER = 1;
    private static final int ID_DONE = 2;
    private static final int RC_PAUSE = 10;
    private static final int RC_STOP = 11;
    private static final int RC_DEADLINE = 20;
    private static final int MAX_SHARE_CHARS = 400_000;

    private Bridge() {
    }

    // ------------------------------------------------------------------ timer

    /** Notificação persistente com cronômetro do sistema + alarme no fim (RF18–RF20). */
    public static void showTimer(Context ctx, String tag, int color, long startMs, long deadlineMs,
                                 boolean overtime, boolean sound, boolean vibrate) {
        ensureChannels(ctx);
        long now = System.currentTimeMillis();
        boolean counting = deadlineMs > now;
        String text = counting
                ? "Termina às " + DateFormat.getTimeFormat(ctx).format(new Date(deadlineMs))
                : "Excedente · meta de hoje cumprida";

        Notification.Builder b = new Notification.Builder(ctx, CHANNEL_TIMER)
                .setSmallIcon(R.drawable.ic_notification)
                .setContentTitle(tag)
                .setContentText(text)
                .setColor(0xFF000000 | color)
                .setOngoing(true)
                .setOnlyAlertOnce(true)
                .setShowWhen(true)
                .setWhen(deadlineMs)
                .setUsesChronometer(true)
                .setChronometerCountDown(counting)
                .setCategory(Notification.CATEGORY_STOPWATCH)
                .setVisibility(Notification.VISIBILITY_PUBLIC)
                .setContentIntent(openApp(ctx))
                .addAction(action(ctx, R.drawable.ic_pause, "Pausar", ACTION_PAUSE, RC_PAUSE))
                .addAction(action(ctx, R.drawable.ic_stop, "Parar", ACTION_STOP, RC_STOP));
        notify(ctx, ID_TIMER, b.build());

        cancelAlarm(ctx);
        if (counting) {
            scheduleAlarm(ctx, tag, deadlineMs, sound, vibrate);
        }
    }

    public static void clearTimer(Context ctx) {
        nm(ctx).cancel(ID_TIMER);
        cancelAlarm(ctx);
    }

    /** Aviso de fim (RF20). Som e vibração controlados pelo app (RF34). */
    public static void notifyFinished(Context ctx, String tag, boolean sound, boolean vibrate) {
        ensureChannels(ctx);
        Notification n = new Notification.Builder(ctx, CHANNEL_DONE)
                .setSmallIcon(R.drawable.ic_notification)
                .setContentTitle("Tempo esgotado: " + (tag == null ? "" : tag))
                .setContentText("A meta de hoje para esta tag foi cumprida.")
                .setAutoCancel(true)
                .setCategory(Notification.CATEGORY_ALARM)
                .setContentIntent(openApp(ctx))
                .build();
        notify(ctx, ID_DONE, n);

        if (sound) {
            try {
                Uri uri = RingtoneManager.getDefaultUri(RingtoneManager.TYPE_NOTIFICATION);
                Ringtone r = RingtoneManager.getRingtone(ctx, uri);
                if (r != null) {
                    r.setAudioAttributes(new AudioAttributes.Builder()
                            .setUsage(AudioAttributes.USAGE_NOTIFICATION_EVENT).build());
                    r.play();
                }
            } catch (RuntimeException e) {
                Log.w(TAG, "som", e);
            }
        }
        if (vibrate) {
            Vibrator v = ctx.getSystemService(Vibrator.class);
            if (v != null && v.hasVibrator()) {
                v.vibrate(VibrationEffect.createWaveform(new long[]{0, 350, 200, 350, 200, 350}, -1));
            }
        }
    }

    // ------------------------------------------------------------------ permissões

    public static boolean notificationsAllowed(Context ctx) {
        if (Build.VERSION.SDK_INT >= 33
                && ctx.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED) {
            return false;
        }
        return nm(ctx).areNotificationsEnabled();
    }

    public static void requestNotifications(Activity a) {
        if (notificationsAllowed(a)) {
            return;
        }
        SharedPreferences prefs = a.getSharedPreferences("bridge", Context.MODE_PRIVATE);
        int asked = prefs.getInt("notif_asked", 0);
        if (Build.VERSION.SDK_INT >= 33 && asked < 2) {
            prefs.edit().putInt("notif_asked", asked + 1).apply();
            a.runOnUiThread(() -> a.requestPermissions(new String[]{Manifest.permission.POST_NOTIFICATIONS}, 1));
            return;
        }
        // Já recusado antes (ou Android < 13 com notificações desligadas): abre os ajustes do app.
        Intent i = new Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS)
                .putExtra(Settings.EXTRA_APP_PACKAGE, a.getPackageName());
        a.runOnUiThread(() -> a.startActivity(i));
    }

    public static boolean exactAlarmsAllowed(Context ctx) {
        if (Build.VERSION.SDK_INT < 31) {
            return true;
        }
        AlarmManager am = ctx.getSystemService(AlarmManager.class);
        return am != null && am.canScheduleExactAlarms();
    }

    public static void requestExactAlarms(Activity a) {
        if (Build.VERSION.SDK_INT < 31 || exactAlarmsAllowed(a)) {
            return;
        }
        Intent i = new Intent(Settings.ACTION_REQUEST_SCHEDULE_EXACT_ALARM,
                Uri.parse("package:" + a.getPackageName()));
        a.runOnUiThread(() -> a.startActivity(i));
    }

    // ------------------------------------------------------------------ exportação

    public static void shareText(Context ctx, String title, String text) {
        if (text.length() > MAX_SHARE_CHARS) {
            // Grande demais para uma intent: salva em arquivo na pasta do app.
            File dir = ctx.getExternalFilesDir(Environment.DIRECTORY_DOCUMENTS);
            if (dir == null) {
                dir = ctx.getFilesDir();
            }
            File f = new File(dir, "tempo-sessoes.csv");
            try (FileOutputStream out = new FileOutputStream(f)) {
                out.write(text.getBytes(StandardCharsets.UTF_8));
                toast(ctx, "Exportado para " + f.getAbsolutePath());
            } catch (Exception e) {
                Log.e(TAG, "exportar", e);
                toast(ctx, "Não foi possível exportar.");
            }
            return;
        }
        Intent send = new Intent(Intent.ACTION_SEND)
                .setType("text/plain")
                .putExtra(Intent.EXTRA_SUBJECT, title)
                .putExtra(Intent.EXTRA_TEXT, text);
        Intent chooser = Intent.createChooser(send, title).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        ctx.startActivity(chooser);
    }

    // ------------------------------------------------------------------ auxiliares

    private static NotificationManager nm(Context ctx) {
        return ctx.getSystemService(NotificationManager.class);
    }

    private static void ensureChannels(Context ctx) {
        NotificationManager nm = nm(ctx);
        if (nm.getNotificationChannel(CHANNEL_TIMER) == null) {
            NotificationChannel c = new NotificationChannel(CHANNEL_TIMER, "Timer em andamento",
                    NotificationManager.IMPORTANCE_LOW);
            c.setDescription("Mostra o timer que está rodando, com botões para pausar e parar.");
            c.setShowBadge(false);
            nm.createNotificationChannel(c);
        }
        if (nm.getNotificationChannel(CHANNEL_DONE) == null) {
            NotificationChannel c = new NotificationChannel(CHANNEL_DONE, "Tempo esgotado",
                    NotificationManager.IMPORTANCE_HIGH);
            c.setDescription("Avisa quando o tempo de uma tag acaba.");
            c.setSound(null, null);
            c.enableVibration(false);
            nm.createNotificationChannel(c);
        }
    }

    private static void notify(Context ctx, int id, Notification n) {
        if (!notificationsAllowed(ctx)) {
            return;
        }
        try {
            nm(ctx).notify(id, n);
        } catch (SecurityException e) {
            Log.w(TAG, "notificação", e);
        }
    }

    private static PendingIntent openApp(Context ctx) {
        Intent i = ctx.getPackageManager().getLaunchIntentForPackage(ctx.getPackageName());
        if (i == null) {
            i = new Intent();
        }
        i.setFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_RESET_TASK_IF_NEEDED);
        return PendingIntent.getActivity(ctx, 0, i,
                PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
    }

    private static Notification.Action action(Context ctx, int icon, String label, String act, int rc) {
        Intent i = new Intent(ctx, TimerReceiver.class).setAction(act);
        PendingIntent pi = PendingIntent.getBroadcast(ctx, rc, i,
                PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        return new Notification.Action.Builder(Icon.createWithResource(ctx, icon), label, pi).build();
    }

    private static PendingIntent deadlineIntent(Context ctx, String tag, boolean sound, boolean vibrate) {
        Intent i = new Intent(ctx, TimerReceiver.class)
                .setAction(ACTION_DEADLINE)
                .putExtra(EXTRA_TAG, tag)
                .putExtra(EXTRA_SOUND, sound)
                .putExtra(EXTRA_VIBRATE, vibrate);
        return PendingIntent.getBroadcast(ctx, RC_DEADLINE, i,
                PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
    }

    private static void scheduleAlarm(Context ctx, String tag, long atMs, boolean sound, boolean vibrate) {
        AlarmManager am = ctx.getSystemService(AlarmManager.class);
        if (am == null) {
            return;
        }
        PendingIntent pi = deadlineIntent(ctx, tag, sound, vibrate);
        try {
            if (exactAlarmsAllowed(ctx)) {
                am.setExactAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, atMs, pi);
            } else {
                am.setAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, atMs, pi);
            }
        } catch (SecurityException e) {
            am.setAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, atMs, pi);
        }
    }

    private static void cancelAlarm(Context ctx) {
        AlarmManager am = ctx.getSystemService(AlarmManager.class);
        if (am != null) {
            am.cancel(deadlineIntent(ctx, null, false, false));
        }
    }

    private static void toast(Context ctx, String msg) {
        new android.os.Handler(ctx.getMainLooper()).post(
                () -> Toast.makeText(ctx, msg, Toast.LENGTH_LONG).show());
    }
}
