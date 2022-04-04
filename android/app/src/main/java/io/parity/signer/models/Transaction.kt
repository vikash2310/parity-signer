package io.parity.signer.models

import io.parity.signer.ButtonID
import org.json.JSONArray
import org.json.JSONObject

/**
 * Turn backend payload section into nice sorted array of transaction cards
 */
fun JSONObject.parseTransaction(): JSONArray {
	val author = this.optJSONArray("author") ?: JSONArray()
	val error = this.optJSONArray("error") ?: JSONArray()
	val extensions =
		this.optJSONArray("extensions") ?: JSONArray()
	val importingDerivations = this.optJSONArray("importing_derivations") ?: JSONArray()
	val message = this.optJSONArray("message") ?: JSONArray()
	val meta = this.optJSONArray("meta") ?: JSONArray()
	val method = (this.optJSONArray("method") ?: JSONArray())
	val newSpecs = (this.optJSONArray("new_specs") ?: JSONArray())
	val verifier = (this.optJSONArray("verifier") ?: JSONArray())
	val warning = this.optJSONArray("warning") ?: JSONArray()
	val typesInfo =
		this.optJSONArray("types_info") ?: JSONArray()

	return sortCards(
		concatJSONArray(
			author,
			error,
			extensions,
			importingDerivations,
			message,
			meta,
			method,
			newSpecs,
			verifier,
			warning,
			typesInfo
		)
	)
}

enum class TransactionType {
	sign,
	stub,
	read,
	import_derivations,
	done;
}

fun SignerDataModel.signTransaction(comment: String) {
	authentication.authenticate(activity) {
		val seedPhrase = getSeed(
			screenData.value?.optJSONObject("author_info")
				?.optString("seed") ?: ""
		)
		if (seedPhrase.isNotBlank()) {
			pushButton(ButtonID.GoForward, comment.encode64(), seedPhrase)
		}
	}
}

fun SignerDataModel.signSufficientCrypto(identity: JSONObject) {
	authentication.authenticate(activity) {
		val seedPhrase = getSeed(
			identity.optString("seed_name")
		)
		if (seedPhrase.isNotBlank()) {
			pushButton(
				ButtonID.GoForward,
				identity.optString("address_key"),
				seedPhrase
			)
		}
	}
}
