//
//  KeyDetailsMulti.swift
//  NativeSigner
//
//  Created by Alexander Slesarev on 6.1.2022.
//

import SwiftUI

struct KeyDetailsMulti: View {
    @EnvironmentObject var data: SignerDataModel
    @GestureState private var dragOffset = CGSize.zero
    @State var offset: CGFloat = 0
    @State var showDetails = false
    var content: MKeyDetailsMulti
    var body: some View {
        ScrollView {
            VStack {
                AddressCard(address: content.intoAddress())
                NetworkCard(title: content.network_title, logo: content.network_logo)
                Image(uiImage: UIImage(data: Data(fromHexEncodedString: content.qr) ?? Data()) ?? UIImage())
                    .resizable()
                    .aspectRatio(contentMode: .fit).padding(12)
                    .offset(x: offset, y:0)
                    .onAppear{
                        offset = 0
                    }
                Text("Key " + content.current_number + " out of " + content.out_of)
            }
        }
        .gesture(
            DragGesture()
                .onChanged {drag in
                    self.offset = drag.translation.width
                }
                .onEnded {drag in
                    self.offset = 0
                    if abs(drag.translation.height) > 200 {
                        showDetails.toggle()
                    } else {
                        if drag.translation.width > 20 {
                            data.pushButton(buttonID: .NextUnit)
                        }
                        if drag.translation.width < -20 {
                            data.pushButton(buttonID: .PreviousUnit)
                        }
                    }
                }
        )
    }
}

/*
 struct KeyDetailsMulti_Previews: PreviewProvider {
 static var previews: some View {
 KeyDetailsMulti()
 }
 }
 */
