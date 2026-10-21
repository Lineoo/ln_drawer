package dev.linn.lndrawer

import android.app.NativeActivity
import android.os.Bundle
import android.util.Log

class MainActivity : NativeActivity() {
    companion object {
        private const val TAG = "ln_drawer"

        init {
            System.loadLibrary("ln_drawer")
        }
    }

    external fun nativeInit(filesDir: String): String

    fun onNativeReady(message: String) {
        Log.i(TAG, "JNI Rust -> Kotlin: $message")
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        val pong = nativeInit(filesDir.absolutePath)
        Log.i(TAG, "JNI Kotlin -> Rust: $pong")
        super.onCreate(savedInstanceState)
    }
}
