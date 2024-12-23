package com.example.clipboardsync

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context.CLIPBOARD_SERVICE
import android.os.Bundle
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.ClickableText
import androidx.compose.material3.Scaffold
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat.getSystemService
import androidx.core.content.ContextCompat.getSystemServiceName
import com.example.clipboardsync.ui.theme.ClipboardSyncTheme
import io.ktor.client.HttpClient
import io.ktor.client.engine.cio.CIO
import io.ktor.client.plugins.websocket.WebSockets
import io.ktor.client.plugins.websocket.webSocket
import io.ktor.websocket.Frame
import io.ktor.websocket.readText
import java.security.KeyPair
import java.security.KeyPairGenerator
import java.security.KeyStore
import java.security.PublicKey
import kotlin.io.encoding.Base64
import kotlin.io.encoding.ExperimentalEncodingApi

object KeyStoreUtils {
    private val ANDROID_KEY_STORE = "AndroidKeyStore"
    private val alias = "clipboard-sync-key"

    private val keyStore = KeyStore.getInstance(ANDROID_KEY_STORE).apply {
        load(null)
    }

    fun getKeyPair(): KeyPair {
        val entry = keyStore.getEntry(alias, null)
        return if (entry == null) {
            createKeyPair()
        } else {
            val privateKey = (entry as KeyStore.PrivateKeyEntry).privateKey
            val publicKey = keyStore.getCertificate(alias).publicKey
            KeyPair(publicKey, privateKey)
        }
    }

    private fun createKeyPair(): KeyPair {
        val keyGenerator = KeyPairGenerator
            .getInstance(KeyProperties.KEY_ALGORITHM_RSA, ANDROID_KEY_STORE)

        keyGenerator.initialize(
            KeyGenParameterSpec.Builder(
                alias,
                KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT
            )
                .setDigests(KeyProperties.DIGEST_SHA256, KeyProperties.DIGEST_SHA512)
                .setKeySize(2048)
                .build()
        )

        return keyGenerator.generateKeyPair()
    }
}

class MainActivity : ComponentActivity() {
    @OptIn(ExperimentalEncodingApi::class)
    override fun onCreate(savedInstanceState: Bundle?) {
        val keyPair = KeyStoreUtils.getKeyPair()

        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            ClipboardSyncTheme {
                Scaffold(
                    modifier = Modifier
                        .padding(16.dp)
                        .fillMaxSize()
                ) { innerPadding ->
                    Greeting(
                        key = keyPair.public,
                        modifier = Modifier.padding(innerPadding)
                    )
                }
            }
        }
    }
}

@OptIn(ExperimentalEncodingApi::class)
@Composable
fun Greeting(key: PublicKey, modifier: Modifier) {
    val str_key = Base64.encode(key.encoded)
    val clipboardManager = requireNotNull(LocalContext.current.getSystemService(CLIPBOARD_SERVICE)) as ClipboardManager

    ClickableText(text = AnnotatedString(str_key), modifier = modifier) {
        clipboardManager.setPrimaryClip(ClipData.newPlainText("", str_key))
        println(str_key)
    }
}

private suspend fun connect() {
    val client = HttpClient(CIO).config { install(WebSockets) }
    val urlString = "ws://${"127.0.0.1:5200"}"

    client.webSocket(urlString = urlString) {
        while (true) {
            val othersMessage = incoming.receive() as? Frame.Binary
            println("xxx")
        }
    }
    client.close()
}