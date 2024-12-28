package com.example.clipboardsync

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Context.CLIPBOARD_SERVICE
import androidx.work.CoroutineWorker
import androidx.work.OneTimeWorkRequest
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.WorkerParameters
import androidx.work.workDataOf
import io.ktor.client.HttpClient
import io.ktor.client.engine.cio.CIO
import io.ktor.client.plugins.websocket.WebSockets
import io.ktor.client.plugins.websocket.webSocket
import io.ktor.websocket.Frame
import io.ktor.websocket.readBytes
import io.ktor.websocket.send
import kotlinx.coroutines.launch
import java.security.KeyPair
import javax.crypto.Cipher
import kotlin.io.encoding.Base64
import kotlin.io.encoding.ExperimentalEncodingApi

class SyncWorker(private val context: Context, params: WorkerParameters): CoroutineWorker(context, params) {
    companion object {
        @OptIn(ExperimentalEncodingApi::class)
        fun create(peersData: PeersData): OneTimeWorkRequest =
            OneTimeWorkRequestBuilder<SyncWorker>()
                .setInputData(workDataOf(
                    "clientKeys" to peersData.keys.map { Base64.encode(it.encoded) }.toTypedArray(),
                ))
                .build()
    }
    
    override suspend fun doWork(): Result {
        val keyPair = KeyStoreUtils.getKeyPair()
        println("keyPair: $keyPair")

        val clientKeys = inputData.getStringArray("clientKeys")!!
        val peersData = PeersData(clientKeys)

        val clipboardManager = requireNotNull(context.getSystemService(CLIPBOARD_SERVICE)) as ClipboardManager

        syncClipboard(keyPair, peersData, clipboardManager)

        return Result.success()
    }

    private suspend fun syncClipboard(keyPair: KeyPair, peersData: PeersData, clipboardManager: ClipboardManager) {
        val client = HttpClient(CIO).config { install(WebSockets) }
        //val urlString = "ws://${"130.61.88.218:5200"}"
        val urlString = "ws://${"10.0.2.2:5200"}"

        val id = digest(keyPair.public.encoded)
        val privateKey = keyPair.private

        val cipher = Cipher.getInstance("RSA/ECB/PKCS1Padding")
        cipher.init(Cipher.DECRYPT_MODE, privateKey)

        var currentClipboard = clipboardManager.primaryClip?.getItemAt(0)?.text.toString()
        client.webSocket(urlString = urlString) {
            send(id)
            val scope = this

            clipboardManager.addPrimaryClipChangedListener {
                val message = clipboardManager.primaryClip?.getItemAt(0)?.text.toString()

                if (message == currentClipboard) {
                    return@addPrimaryClipChangedListener
                }

                currentClipboard = message
                peersData.encryptForAll(message)
                    .forEach { (id, message) ->
                        scope.launch {
                            send(Frame.Text(id))
                            send(Frame.Binary(true, message))
                        }
                    }
            }

            while (true) {
                val othersMessage = incoming.receive() as? Frame.Binary
                othersMessage?.readBytes().let { rawMessage ->
                    if (rawMessage == null) {
                        return@let
                    }

                    val decryptedBytes = cipher.doFinal(rawMessage)
                    val message = String(decryptedBytes)

                    if (message == currentClipboard) {
                        return@let
                    }

                    currentClipboard = message
                    clipboardManager.setPrimaryClip(ClipData.newPlainText("", message))
                }
            }
        }
        println("connection closed")
        client.close()
    }
}