package io.parity.signer.components.transactionCards

import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.width
import androidx.compose.material.MaterialTheme
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import io.parity.signer.ui.theme.Text400
import io.parity.signer.ui.theme.Text600
import org.json.JSONObject

@Composable
fun TCTip(payload: JSONObject) {
	Row {
		Text("Tip:", color = MaterialTheme.colors.Text400)
		Spacer(modifier = Modifier.width(16.dp))
		Text(
			payload.getString("amount") + " " + payload.getString("units"),
			color = MaterialTheme.colors.Text600
		)
	}
}
