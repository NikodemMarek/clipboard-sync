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
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.ClickableText
import androidx.compose.material3.Button
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.unit.dp
import androidx.lifecycle.lifecycleScope
import com.example.clipboardsync.ui.theme.ClipboardSyncTheme
import io.ktor.client.HttpClient
import io.ktor.client.engine.cio.CIO
import io.ktor.client.plugins.websocket.WebSockets
import io.ktor.client.plugins.websocket.webSocket
import io.ktor.websocket.Frame
import io.ktor.websocket.readBytes
import io.ktor.websocket.send
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import kotlinx.coroutines.supervisorScope
import java.security.KeyFactory
import java.security.KeyPair
import java.security.KeyPairGenerator
import java.security.KeyStore
import java.security.MessageDigest
import java.security.PublicKey
import java.security.spec.X509EncodedKeySpec
import javax.crypto.Cipher
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
                .setDigests(KeyProperties.DIGEST_SHA256)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_RSA_PKCS1)
                .setKeySize(2048)
                .build()
        )

        return keyGenerator.generateKeyPair()
    }

    fun removeKey() {
        keyStore.deleteEntry(alias)
    }
}

object ClientsService {
    private val kf = KeyFactory.getInstance("RSA")

    private val clients = mutableMapOf<String, PublicKey>()

    fun addClient(rawKey: String) {
        val pubKey = kf.generatePublic(X509EncodedKeySpec(stringToByteArray(rawKey))) as PublicKey
        val id = keyDigest(pubKey)
        clients[id] = pubKey
    }

    fun removeClient(id: String) {
        clients.remove(id)
    }

    fun getClients(): Map<String, PublicKey> {
        return clients
    }

    fun encryptMessage(message: String, id: String): ByteArray {
        val cipher = Cipher.getInstance("RSA/ECB/PKCS1Padding")
        cipher.init(Cipher.ENCRYPT_MODE, clients[id])
        return cipher.doFinal(message.toByteArray())
    }

    fun encryptForAll(message: String): Map<String, ByteArray> {
        return clients.mapValues { (_, key) ->
            encryptMessage(message, keyDigest(key))
        }
    }
}

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        var keyPair = KeyStoreUtils.getKeyPair()
        val clipboardManager =
            requireNotNull(
                this.getSystemService(
                    CLIPBOARD_SERVICE
                )
            ) as ClipboardManager

        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            ClipboardSyncTheme {
                Scaffold(
                    modifier = Modifier
                        .padding(16.dp)
                        .fillMaxSize()
                ) { innerPadding ->
                    Column {
                        KeyDisplay(
                            key = keyPair.public,
                            modifier = Modifier.padding(innerPadding)
                        )

                        Button(onClick = {
                            lifecycleScope.launch {
                                connect(keyPair, clipboardManager)
                            }
                        }) {
                            Text(text = "start")
                        }

                        Button(onClick = {
                            KeyStoreUtils.removeKey()
                            keyPair = KeyStoreUtils.getKeyPair()
                        }) {
                            Text(text = "regenerate key")
                        }

                        ClientKeys(modifier = Modifier.padding(innerPadding))
                    }
                }
            }
        }
    }
}

@OptIn(ExperimentalEncodingApi::class)
fun keyToString(key: PublicKey): String =
    "-----BEGIN PUBLIC KEY-----\n" + Base64.encode(key.encoded).chunked(64)
        .joinToString("\n") + "\n-----END PUBLIC KEY-----"

@OptIn(ExperimentalEncodingApi::class)
fun stringToByteArray(str: String): ByteArray =
    Base64.decode(
        str
            .replace(" ", "")
            .replace("\n", "")
            .replace("-----BEGINPUBLICKEY-----", "")
            .replace("-----ENDPUBLICKEY-----", "")
            .replace("'", "")
    )

fun keyDigest(key: PublicKey): String =
    bin2hex(MessageDigest.getInstance("SHA-256").digest(key.encoded))

fun bin2hex(data: ByteArray): String {
    val hex = StringBuilder(data.size * 2)
    for (b in data) hex.append(String.format("%02x", b.toInt() and 0xFF))
    return hex.toString()
}

@Composable
fun KeyDisplay(key: PublicKey, modifier: Modifier) {
    val strKey = keyToString(key)
    val digest = keyDigest(key)
    val clipboardManager =
        requireNotNull(LocalContext.current.getSystemService(CLIPBOARD_SERVICE)) as ClipboardManager

    ClickableText(text = AnnotatedString(strKey + "\n\n\n" + digest), modifier = modifier) {
        clipboardManager.setPrimaryClip(ClipData.newPlainText("", strKey))
    }
}

@Composable
fun ClientKeys(modifier: Modifier) {
    var newKeyValue by remember { mutableStateOf("") }

    TextField(value = newKeyValue, onValueChange = {
        newKeyValue = it
    })

    Button(onClick = {
        ClientsService.addClient(newKeyValue)
        newKeyValue = ""
    }) {
        Text(text = "add key")
    }

    ClientsService.getClients().forEach { (id, _) ->
        Text(text = id)
    }
}

suspend fun connect(keyPair: KeyPair, clipboardManager: ClipboardManager) {
    val client = HttpClient(CIO).config { install(WebSockets) }
    //val urlString = "ws://${"130.61.88.218:5200"}"
    val urlString = "ws://${"10.0.2.2:5200"}"

    val id = keyDigest(keyPair.public)
    val privateKey = keyPair.private

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
            ClientsService.encryptForAll(message)
                .forEach { (id, message) ->
                    scope.launch {
                        send(Frame.Text(id))
                        send(Frame.Binary(true, message))
                    }
                }
        }

        while (true) {
            val othersMessage = incoming.receive() as? Frame.Binary
            othersMessage?.readBytes().let { message ->
                if (message == null) {
                    return@let
                }

                val cipher = Cipher.getInstance("RSA/ECB/PKCS1Padding")
                cipher.init(Cipher.DECRYPT_MODE, privateKey)
                val decryptedBytes = cipher.doFinal(message)
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