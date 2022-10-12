/******************************************************************************
* Copyright (C) 2017 - 2021 Xilinx, Inc.  All rights reserved.
* SPDX-License-Identifier: MIT
******************************************************************************/

/*****************************************************************************/
/**
* @file xaxiethernet_fifo_ping_req_example.c
*
* This file contains a Axi Ethernet Ping request example in polled mode.
* This example will generate a ping request for the specified IP address.
* edit SHa: modified example to support AXI FIFO Hardware
*
* @note
*
* The local IP address is set to 10.10.70.6. User needs to update
* LocalIpAddr variable with a free IP address based on the network on which
* this example is to be run.
*
* The Destination IP address is set to 10.10.70.3. User needs to update
* DestIpAddress variable with any valid IP address based on the network on which
* this example is to be run.
*
* The local MAC address is set to 0x000A35030201. User can update LocalMacAddr
* variable with a valid MAC address. The first three bytes contains
* the manufacture ID. 0x000A35 is XILINX manufacture ID.
*
* This program will generate the specified number of ping request packets as
* defined in "NUM_OF_PING_REQ_PKTS".
*
* This example got validated only for SGMII based design's.
*
* Functional guide to example:
*
* - SendArpReqFrame demonstrates the way to send the ARP request packets
*   in the polling mode
* - SendEchoReqFrame demonstrates the way to send the ICMP/ECHO request packets
*   in the polling mode
* - ProcessRecvFrame demonstrates the way to process the received packet.
*   This function sends the echo request packet based on the ARP reply packet.
*
* <pre>
* MODIFICATION HISTORY:
*
* Ver   Who  Date     Changes
* ----- ---- -------- -----------------------------------------------
* 5.5   adk		Initial Release
* </pre>
*
*****************************************************************************/
/***************************** Include Files *********************************/

#include "xaxiethernet_example.h"
#include "xstatus.h"
#include "sleep.h"
#include "xdebug.h"


/************************** Constant Definitions *****************************/

/*
 * The following constants map to the XPAR parameters created in the
 * xparameters.h file. They are defined here such that a user can easily
 * change all the needed parameters in one place.
 */
#define AXIETHERNET_DEVICE_ID	XPAR_AXIETHERNET_0_DEVICE_ID
#define FIFO_DEVICE_ID		XPAR_AXI_FIFO_0_DEVICE_ID

/*
 * Change this parameter to limit the number of ping requests sent by this
 * program.
 */
#define NUM_OF_PING_REQ_PKTS	10	/* Number of ping req it generates */

#define RXBD_CNT			1024	/* Number of RxBDs to use */
#define TXBD_CNT			1024	/* Number of TxBDs to use */
#define BD_ALIGNMENT			64	/* Byte alignment of BDs */

#define ECHO_REPLY		0x00	/* Echo reply */
#define HW_TYPE			0x01	/* Hardware type (10/100 Mbps) */
#define ARP_REQUEST 		0x01	/* ARP Request bits in Rx packet */
#define ARP_REPLY 		0x02 	/* ARP status bits indicating reply */
#define IDEN_NUM		0x02	/* ICMP identifier number */
#define IP_VERSION		0x0604	/* IP version ipv4/ipv6 */
#define BROADCAST_ADDR 		0xFFFF 	/* Broadcast Address */
#define CORRECT_CHECKSUM_VALUE	0xFFFF	/* Correct checksum value */
#define ARP_REQ_PKT_SIZE	0x2A	/* ARP request packet size */
#define ICMP_PKT_SIZE 		0x4A	/* ICMP packet length 74 Bytes
					including Src and dest MAC Add */
#define IP_ADDR_SIZE		4	/* IP Address size in Bytes */
#define NUM_RX_PACK_CHECK_REQ	10	/* Max num of Rx pack to be checked
					before sending another request */
#define NUM_PACK_CHECK_RX_PACK	8000000	/* Max number of pack to be checked
					before to identify a Rx packet */
#define DELAY			1000000 /* Used to introduce delay */

/*
 * Definitions for the locations and length of some of the fields in a
 * IP packet. The lengths are defined in Half-Words (2 bytes).
 */

#define SRC_MAC_ADDR_LOC	3	/* Src MAC address location */
#define MAC_ADDR_LEN 		3	/* MAC address length */
#define ETHER_PROTO_TYPE_LOC	6	/* Ethernet Proto type loc */

#define IP_ADDR_LEN 		2	/* Size of IP address */
#define IP_START_LOC 		7	/* IP header start location */
#define IP_HEADER_INFO_LEN	7	/* IP header information length */
#define IP_HEADER_LEN 		10 	/* IP header length */
#define IP_CHECKSUM_LOC		12	/* IP header checksum location */
#define IP_REQ_SRC_IP_LOC 	13	/* Src IP add loc of ICMP req */
#define IP_REQ_DEST_IP_LOC	15	/* Dest IP add loc of ICMP req */

#define ICMP_KNOWN_DATA_LEN	16	/* ICMP known data length */
#define ICMP_ECHO_FIELD_LOC 	17	/* Echo field loc */
#define ICMP_DATA_START_LOC 	17	/* Data field start location */
#define ICMP_DATA_LEN 		18	/* ICMP data length */
#define ICMP_DATA_CHECKSUM_LOC	18	/* ICMP data checksum location */
#define ICMP_IDEN_FIELD_LOC	19	/* Identifier field loc */
#define ICMP_DATA_LOC 		19	/* ICMP data loc including
					identifier number and sequence number */
#define ICMP_SEQ_NO_LOC		20	/* sequence number location */
#define ICMP_DATA_FIELD_LEN 	20 	/* Data field length */
#define ICMP_KNOWN_DATA_LOC	21	/* ICMP known data start loc */

#define ARP_REQ_STATUS_LOC 	10	/* ARP request loc */
#define ARP_REQ_SRC_IP_LOC 	14	/* Src IP add loc of ARP req Packet */

#define RXBD_SPACE_BYTES RXBD_CNT * 64 * 16
#define TXBD_SPACE_BYTES TXBD_CNT * 64 * 16

/*
 * General Ethernet Definitions
 */
#define XAE_ETHER_PROTO_TYPE_IP         0x0800  /**< IP Protocol */
#define XAE_ETHER_PROTO_TYPE_ARP        0x0806  /**< ARP Protocol */
#define XAE_ETHER_PROTO_TYPE_VLAN       0x8100  /**< VLAN Tagged */
#define XAE_ARP_PACKET_SIZE             28      /**< Max ARP packet size */
#define XAE_HEADER_IP_LENGTH_OFFSET     16      /**< IP Length Offset */
#define XAE_VLAN_TAG_SIZE               4       /**< VLAN Tag Size */


#define PHY_R0_CTRL_REG		0
#define PHY_R3_PHY_IDENT_REG	3

#define PHY_R0_RESET         0x8000
#define PHY_R0_LOOPBACK      0x4000
#define PHY_R0_ANEG_ENABLE   0x1000
#define PHY_R0_DFT_SPD_MASK  0x2040
#define PHY_R0_DFT_SPD_10    0x0000
#define PHY_R0_DFT_SPD_100   0x2000
#define PHY_R0_DFT_SPD_1000  0x0040
#define PHY_R0_ISOLATE       0x0400

/* Marvel PHY 88E1111 Specific definitions */
#define PHY_R20_EXTND_CTRL_REG	20
#define PHY_R27_EXTND_STS_REG	27

#define PHY_R20_DFT_SPD_10    	0x20
#define PHY_R20_DFT_SPD_100   	0x50
#define PHY_R20_DFT_SPD_1000  	0x60
#define PHY_R20_RX_DLY		0x80

#define PHY_R27_MAC_CONFIG_GMII      0x000F
#define PHY_R27_MAC_CONFIG_MII       0x000F
#define PHY_R27_MAC_CONFIG_RGMII     0x000B
#define PHY_R27_MAC_CONFIG_SGMII     0x0004

/* Marvel PHY 88E1116R Specific definitions */
#define PHY_R22_PAGE_ADDR_REG	22
#define PHY_PG2_R21_CTRL_REG	21

#define PHY_REG21_10      0x0030
#define PHY_REG21_100     0x2030
#define PHY_REG21_1000    0x0070

/* Marvel PHY flags */
#define MARVEL_PHY_88E1111_MODEL	0xC0
#define MARVEL_PHY_88E1116R_MODEL	0x240
#define PHY_MODEL_NUM_MASK		0x3F0

/* TI PHY flags */
#define TI_PHY_IDENTIFIER		0x2000
#define TI_PHY_MODEL			0x230
#define TI_PHY_CR			0xD
#define TI_PHY_PHYCTRL			0x10
#define TI_PHY_CR_SGMII_EN		0x0800
#define TI_PHY_ADDDR			0xE
#define TI_PHY_CFGR2			0x14
#define TI_PHY_SGMIITYPE		0xD3
#define TI_PHY_CFGR2_SGMII_AUTONEG_EN	0x0080
#define TI_PHY_SGMIICLK_EN		0x4000
#define TI_PHY_CR_DEVAD_EN		0x001F
#define TI_PHY_CR_DEVAD_DATAEN		0x4000
/**************************** Type Definitions *******************************/


/***************** Macros (Inline Functions) Definitions *********************/


/************************** Function Prototypes ******************************/

/*
 * Examples
 */
int AxiEthernetPingReqExample(XAxiEthernet *AxiEthernetInstancePtr,
				  XLlFifo *FifoInstancePtr,
			      u16 AxiEthernetDeviceId);

void SendArpReqFrame(XLlFifo *FifoInstancePtr);

void SendEchoReqFrame(XLlFifo *FifoInstancePtr);

int ProcessRecvFrame(XLlFifo *FifoInstancePtr, u32 FrameLength);

int PhySetup(XAxiEthernet *AxiEthernetInstancePtr);

static u16 CheckSumCalculation(u16 *RxFramePtr16, int StartLoc, int Length);

static int CompareData(u16 *LhsPtr, u16 *RhsPtr, int LhsLoc, int RhsLoc,
			int Count);

/************************** Variable Definitions *****************************/

/*
 * Set up a local MAC address.
 */
static u8 LocalMacAddr[6] =
{
	0x00, 0x0A, 0x35, 0x03, 0x02, 0x01

};

/*
 * The IP address was set to 172.16.63.121. User need to set a free IP address
 * based on the network on which this example is to be run.
 */
static u8 LocalIpAddress[IP_ADDR_SIZE] =
{
	192, 168, 42, 243
};

/*
 * Set up a Destination IP address. Currently it is set to 172.16.63.61.
 */
static u8 DestIpAddress[IP_ADDR_SIZE] =
{
	192, 168, 42, 11
};

static u16 DestMacAddr[MAC_ADDR_LEN]; 	/* Destination MAC Address */


/*
 * Known data transmitted in Echo request.
 */
u16 IcmpData[ICMP_KNOWN_DATA_LEN] =
{
	0x6162,	0x6364,	0x6566, 0x6768, 0x696A,	0x6B6C, 0x6D6E,	0x6F70,
	0x7172, 0x7374, 0x7576, 0x7761, 0x6263,	0x6465, 0x6667,	0x6869
};

/*
 * IP header information -- each field has its own significance.
 * Icmp type, ipv4 typelength, packet length, identification field
 * Fragment type, time to live and ICM, checksum.
 */
u16 IpHeaderInfo[IP_HEADER_INFO_LEN] =
{
	//0x0800,	0x4500, 0x003C,	0x5566,	0x0000,	0x8001, 0x0000
	0x0800,	0x4500, 0x003C,	0x5566,	0x4000,	0x4001, 0x0000
};

/*
 * Variable used to indicate the length of the received frame.
 */
volatile u32 RecvFrameLength;
UINTPTR TxBuffPtr;
UINTPTR RxBuffPtr;
volatile int FramesTx;
volatile int FramesRx;
volatile int TxCount;
volatile int RxCount;
//volatile int Padding;	/* For 1588 Packets we need to pad 8 bytes time stamp value */

/*
 * Variable used to indicate the sequence number of the ICMP(echo) packet.
 */
int SeqNum;

/*
 * Variable used to indicate the number of ping request packets to be send.
 */
int NumOfPingReqPkts;

/****************************************************************************/
/**
*
* This function is the main function of the Ping Request example in polled mode.
*
* @param	None.
*
* @return	XST_FAILURE to indicate failure, otherwise it will return
*		XST_SUCCESS after sending specified number of packets as
*		defined in "NUM_OF_PING_REQ_PKTS" .
*
* @note		None.
*
*****************************************************************************/
int main_fifo_ping_req_example()
{
	int Status;

#ifndef NDEBUG
	Xil_AssertSetCallback(axi_ethernet_assert_callback);
#endif

	AxiEthernetUtilErrorTrap("Run the AxiEthernet Ping request example...\n");

	/*
	 * Run the AxiEthernet Ping request example.
	 */
	Status = AxiEthernetPingReqExample(&AxiEthernetInstance,
			 &FifoInstance,
		     AXIETHERNET_DEVICE_ID);
	if (Status != XST_SUCCESS) {
		AxiEthernetUtilErrorTrap("Axi Ethernet ping request Example Failed\n");
		return XST_FAILURE;
	}

	AxiEthernetUtilErrorTrap("Successfully ran Axi Ethernet ping request Example\n");
	return XST_SUCCESS;
}

/*****************************************************************************/
/**
*
* The entry point for the AxiEthernet driver to ping request example in polled
* mode. This function will generate specified number of request packets as
* defined in "NUM_OF_PING_REQ_PKTS.
*
* @param	AxiEthernetInstancePtr is a pointer to the instance of the
*		AxiEthernet component.
* @param	FifoInstancePtr is a pointer to the instance of the AXI FIFO
*		component.
* @param	AxiEthernetDeviceId is Device ID of the Axi Ethernet Device ,
*		typically XPAR_<AXIETHERNET_instance>_DEVICE_ID value from
*		xparameters.h.
*
* @return	XST_FAILURE to indicate failure, otherwise it will return
*		XST_SUCCESS.
*
* @note		AXI FIFO hardware must be initialized before initializing
*		AxiEthernet. Since AXI FIFO reset line is connected to the
*		AxiEthernet reset line, a reset of AXI FIFO hardware during its
*		initialization would reset AxiEthernet.
*
******************************************************************************/
int AxiEthernetPingReqExample(XAxiEthernet *AxiEthernetInstancePtr,
				  XLlFifo *FifoInstancePtr,
			      u16 AxiEthernetDeviceId)
{
	int Status;
	int Index;
	int Count;
	int EchoReplyStatus;
	XAxiEthernet_Config *MacCfgPtr;
	SeqNum = 0;
	RecvFrameLength = 0;
	EchoReplyStatus = XST_FAILURE;
	NumOfPingReqPkts = NUM_OF_PING_REQ_PKTS;

	AxiEthernetResetDevice();

	/*
	 *  Get the configuration of AxiEthernet hardware.
	 */
	MacCfgPtr = XAxiEthernet_LookupConfig(AxiEthernetDeviceId);

    /*
	 * Check whether AXIFIFO is present or not
	 */
	if(MacCfgPtr->AxiDevType != XPAR_AXI_FIFO) {
		AxiEthernetUtilErrorTrap
			("Device HW not configured for FIFO mode\n");
		return XST_FAILURE;
	}

    XLlFifo_Initialize(FifoInstancePtr, MacCfgPtr->AxiDevBaseAddress);

	/*
	 * Initialize AxiEthernet hardware.
	 */
	Status = XAxiEthernet_CfgInitialize(AxiEthernetInstancePtr, MacCfgPtr,
					MacCfgPtr->BaseAddress);
	if (Status != XST_SUCCESS) {
		AxiEthernetUtilErrorTrap("Error in initialize");
		return XST_FAILURE;
	}

	if (MacCfgPtr->Enable_1588)
		Padding = 8;

	AxiEthernetUtilErrorTrap("Cfg init success\n");
#if defined(__aarch64__)
        Xil_SetTlbAttributes((UINTPTR)TxBdSpace, NORM_NONCACHE | INNER_SHAREABLE);
        Xil_SetTlbAttributes((UINTPTR)RxBdSpace, NORM_NONCACHE | INNER_SHAREABLE);
#endif

	TxCount = 0;
	RxCount = 0;
	TxBuffPtr = (UINTPTR) &TxFrame;
	RxBuffPtr = (UINTPTR) &RxFrame;

	/*
	 * Set the MAC address
	 */
	Status = XAxiEthernet_SetMacAddress(AxiEthernetInstancePtr,
					LocalMacAddr);
	if (Status != XST_SUCCESS) {
	      AxiEthernetUtilErrorTrap("Error setting MAC address");
	      return XST_FAILURE;
	}

	PhySetup(AxiEthernetInstancePtr);

	/*
	 * Setting the operating speed of the MAC needs a delay.  There
	 * doesn't seem to be register to poll, so please consider this
	 * during your application design.
	 */
	 sleep(2);

	/*
	 * Make sure Tx and Rx are enabled
	 */
	Status = XAxiEthernet_SetOptions(AxiEthernetInstancePtr,
				         XAE_RECEIVER_ENABLE_OPTION |
					 XAE_TRANSMITTER_ENABLE_OPTION);
	if (Status != XST_SUCCESS) {
		AxiEthernetUtilErrorTrap("Error setting options");
		return XST_FAILURE;
	}


	/*
	 * Start the Axi Ethernet and enable its ERROR interrupts
	 */
	XAxiEthernet_Start(AxiEthernetInstancePtr);

	/*
	 * Empty any existing receive frames.
	 */
	while (NumOfPingReqPkts--) {

		/*
		 * Introduce delay.
		 */
		Count = DELAY;
		while (Count--) {
		}

		/*
		 * Send an ARP or an ICMP packet based on receive packet.
		 */
		if (SeqNum == 0) {
			AxiEthernetUtilErrorTrap("Send an ARP request packet");
			SendArpReqFrame(FifoInstancePtr);
		} else {
			AxiEthernetUtilErrorTrap("Send an ICMP ping request packet");
			SendEchoReqFrame(FifoInstancePtr);
		}

		/*
		 * Check next 10 packets for the correct reply.
		 */
		Index = NUM_RX_PACK_CHECK_REQ;
		while (Index--) {

			/*
			 * Wait for a Receive packet.
			 */
			Count = NUM_PACK_CHECK_RX_PACK;

			switch (AxiEthernetPollForRxStatus()) {
			case XST_SUCCESS:	/* Got a successful receive status */
				xdbg_printf(XDBG_DEBUG_GENERAL,
					"Got a successful receive status at Packet No: {}\n",
					NUM_PACK_CHECK_RX_PACK - Index);
				break;

			case XST_NO_DATA:	/* Timed out */
				AxiEthernetUtilErrorTrap("Rx timeout");
				return XST_FAILURE;
				break;

			default:	/* Some other error */
				AxiEthernetResetDevice();
				return XST_FAILURE;
			}

			while (RecvFrameLength == 0) {
				if (XLlFifo_RxOccupancy(FifoInstancePtr)) {
					RecvFrameLength = XLlFifo_RxGetLen(FifoInstancePtr);
				}

				/*
				 * To avoid infinite loop when no packet is
				 * received.
				 */
				if (Count-- == 0) {
					break;
				}
			}

			/*
			 * Process the Receive frame.
			 */
			if (RecvFrameLength != 0) {
				xdbg_printf(XDBG_DEBUG_GENERAL, "Read and process the received frame\n");
				EchoReplyStatus = ProcessRecvFrame(FifoInstancePtr, RecvFrameLength);
			}
			RecvFrameLength = 0;

			/*
			 * Comes out of loop when an echo reply packet is
			 * received.
			 */
			if (EchoReplyStatus == XST_SUCCESS) {
				break;
			}
		}

		/*
		 * If no echo reply packet is received, it reports
		 * request timed out.
		 */
		if (EchoReplyStatus == XST_FAILURE) {
			AxiEthernetUtilErrorTrap("No echo reply packet received");
			xdbg_printf(XDBG_DEBUG_ERROR, "Packet No: {}\n", NUM_OF_PING_REQ_PKTS - NumOfPingReqPkts);
			xdbg_printf(XDBG_DEBUG_ERROR, " Seq NO {} Request timed out\n", SeqNum);
		}
	}
	return XST_SUCCESS;
}

int PhySetup(XAxiEthernet *AxiEthernetInstancePtr)
{
	u16 PhyReg0;
	u32 PhyAddr;
	u16 status;

	PhyAddr = XPAR_AXIETHERNET_0_PHYADDR;

	/* Clear the PHY of any existing bits by zeroing this out */
	PhyReg0 = 0;
	XAxiEthernet_PhyRead(AxiEthernetInstancePtr, PhyAddr,
				 PHY_R0_CTRL_REG, &PhyReg0);

	PhyReg0 &= (~PHY_R0_ANEG_ENABLE);
	PhyReg0 &= (~PHY_R0_ISOLATE);
	PhyReg0 |= PHY_R0_DFT_SPD_1000;

	sleep(1);
	XAxiEthernet_PhyWrite(AxiEthernetInstancePtr, PhyAddr,
				PHY_R0_CTRL_REG, PhyReg0);

	XAxiEthernet_PhyRead(AxiEthernetInstancePtr, PhyAddr, 1, &status);

	return XST_SUCCESS;
}


int GetBufAddr()
{
	UINTPTR TxBufPtr;

	if (TxCount !=0) {
		TxBufPtr = TxBuffPtr + ICMP_PKT_SIZE;
		TxBuffPtr += ICMP_PKT_SIZE;
	} else {
		TxBufPtr = TxBuffPtr;
	}

	TxCount++;
	return TxBufPtr;
}

int GetRxBufAddr()
{
	UINTPTR RxBufPtr;

	if (RxCount == 0)
		RxBufPtr = RxBuffPtr;
	else {
		RxBuffPtr += XAE_MTU;
		RxBufPtr = RxBuffPtr;
	}
	RxCount++;

	return RxBufPtr;
}

/*****************************************************************************/
/**
*
* This function will send a ARP request packet.
*
* @param	FifoInstancePtr is a pointer to the instance of the FIFO
*		component.
*
* @return	None.
*
* @note		None.
*
******************************************************************************/
void SendArpReqFrame(XLlFifo *FifoInstancePtr)
{
	u16 *TempPtr;
	u16 *TxFramePtr;
	UINTPTR BufAddr;
	int Index, i;

	u32 FifoFreeBytes;

	FramesTx = 0;
	TxFramePtr = (u16 *)(UINTPTR)GetBufAddr();
	BufAddr = (UINTPTR) TxFramePtr;

	if (Padding) {
		for (i = 0 ; i < 4; i++)
			*TxFramePtr++ = 0;
	}

	/*
	 * Add broadcast address.
	 */
	Index = MAC_ADDR_LEN;
	while (Index--) {
		*TxFramePtr++ = BROADCAST_ADDR;
	}

	/*
	 * Add local MAC address.
	 */
	Index = 0;
	TempPtr = (u16 *)LocalMacAddr;
	while (Index < MAC_ADDR_LEN) {
		*TxFramePtr++ = *(TempPtr + Index);
		Index++;
	}

	/*
	 * Add
	 * 	- Ethernet proto type.
	 *	- Hardware Type
	 *	- Protocol IP Type
	 *	- IP version (IPv6/IPv4)
	 *	- ARP Request
	 */
	*TxFramePtr++ = Xil_Htons(XAE_ETHER_PROTO_TYPE_ARP);
	*TxFramePtr++ = Xil_Htons(HW_TYPE);
	*TxFramePtr++ = Xil_Htons(XAE_ETHER_PROTO_TYPE_IP);
	*TxFramePtr++ = Xil_Htons(IP_VERSION);
	*TxFramePtr++ = Xil_Htons(ARP_REQUEST);

	/*
	 * Add local MAC address.
	 */
	Index = 0;
	TempPtr = (u16 *)LocalMacAddr;
	while (Index < MAC_ADDR_LEN) {
		*TxFramePtr++ = *(TempPtr + Index);
		Index++;
	}

	/*
	 * Add local IP address.
	 */
	Index = 0;
	TempPtr = (u16 *)LocalIpAddress;
	while (Index < IP_ADDR_LEN) {
		*TxFramePtr++ = *(TempPtr + Index);
		Index++;
	}

	/*
	 * Fills 6 bytes of information with zeros as per protocol.
	 */
	Index = 0;
	while (Index < 3) {
		*TxFramePtr++ = 0x0000;
		Index++;
	}

	/*
	 * Add Destination IP address.
	 */
	Index = 0;
	TempPtr = (u16 *)DestIpAddress;
	while (Index < IP_ADDR_LEN) {
		*TxFramePtr++ = *(TempPtr + Index);
		Index++;
	}

	/*
	 * Transmit the Frame.
	 */
	//Wait for enough room in FIFO to become available
	do {
		FifoFreeBytes = XLlFifo_TxVacancy(FifoInstancePtr);
	} while (FifoFreeBytes < ARP_REQ_PKT_SIZE);

	//Write the frame data to FIFO
	XLlFifo_Write(FifoInstancePtr, (void*)BufAddr, ARP_REQ_PKT_SIZE);

	//Initiate transmit
	XLlFifo_TxSetLen(FifoInstancePtr, ARP_REQ_PKT_SIZE);

	//Wait for status of the transmitted packet
	switch (AxiEthernetPollForTxStatus()) {
	case XST_SUCCESS:/* Got a successful transmit status */
		break;

	case XST_NO_DATA:	/* Timed out */
		AxiEthernetUtilErrorTrap("Tx timeout");
		break;

	default:		/* Some other error */
		break;
	}

}

/*****************************************************************************/
/**
*
* This function will send a Echo request packet.
*
* @param	FifoInstancePtr is a pointer to the instance of the FIFO
*		component.
*
* @return	None.
*
* @note		None.
*
******************************************************************************/
void SendEchoReqFrame(XLlFifo *FifoInstancePtr)
{
	u16 *TempPtr;
	u16 *TxFramePtr;
	UINTPTR BufAddr;
	u16 CheckSum;
	int Index, i;

	u32 FifoFreeBytes;

	FramesTx = 0;
	TxFramePtr = (u16 *)(UINTPTR)GetBufAddr();
	BufAddr = (UINTPTR) TxFramePtr;

	if (Padding) {
		for (i = 0 ; i < 4; i++)
			*TxFramePtr++ = 0;
	}

	/*
	 * Add Destination MAC Address.
	 */
	Index = MAC_ADDR_LEN;
	while (Index--) {
		*(TxFramePtr + Index) = *(DestMacAddr + Index);
	}

	/*
	 * Add Source MAC Address.
	 */
	Index = MAC_ADDR_LEN;
	TempPtr = (u16 *)LocalMacAddr;
	while (Index--) {
		*(TxFramePtr + (Index + SRC_MAC_ADDR_LOC )) =
							*(TempPtr + Index);
	}

	/*
	 * Add IP header information.
	 */
	Index = IP_START_LOC;
	while (Index--) {
		*(TxFramePtr + (Index + ETHER_PROTO_TYPE_LOC )) =
				Xil_Htons(*(IpHeaderInfo + Index));
	}

	/*
	 * Add Source IP address.
	 */
	Index = IP_ADDR_LEN;
	TempPtr = (u16 *)LocalIpAddress;
	while (Index--) {
		*(TxFramePtr + (Index + IP_REQ_SRC_IP_LOC )) =
						*(TempPtr + Index);
	}

	/*
	 * Add Destination IP address.
	 */
	Index = IP_ADDR_LEN;
	TempPtr = (u16 *)DestIpAddress;
	while (Index--) {
		*(TxFramePtr + (Index + IP_REQ_DEST_IP_LOC )) =
						*(TempPtr + Index);
	}

	/*
	 * Checksum is calculated for IP field and added in the frame.
	 */
	CheckSum = CheckSumCalculation((u16 *)BufAddr, IP_START_LOC,
							IP_HEADER_LEN);
	CheckSum = ~CheckSum;
	*(TxFramePtr + IP_CHECKSUM_LOC) = Xil_Htons(CheckSum);

	/*
	 * Add echo field information.
	 */
	*(TxFramePtr + ICMP_ECHO_FIELD_LOC) = Xil_Htons(XAE_ETHER_PROTO_TYPE_IP);

	/*
	 * Checksum value is initialized to zeros.
	 */
	*(TxFramePtr + ICMP_DATA_LEN) = 0x0000;

	/*
	 * Add identifier and sequence number to the frame.
	 */
	*(TxFramePtr + ICMP_IDEN_FIELD_LOC) = (IDEN_NUM);
	*(TxFramePtr + (ICMP_IDEN_FIELD_LOC + 1)) = Xil_Htons((u16)(++SeqNum));

	/*
	 * Add known data to the frame.
	 */
	Index = ICMP_KNOWN_DATA_LEN;
	while (Index--) {
		*(TxFramePtr + (Index + ICMP_KNOWN_DATA_LOC)) =
				Xil_Htons(*(IcmpData + Index));
	}

	/*
	 * Checksum is calculated for Data Field and added in the frame.
	 */
	CheckSum = CheckSumCalculation((u16 *)BufAddr, ICMP_DATA_START_LOC,
						ICMP_DATA_FIELD_LEN );
	CheckSum = ~CheckSum;
	*(TxFramePtr + ICMP_DATA_CHECKSUM_LOC) = Xil_Htons(CheckSum);

	/*
	 * Transmit the Frame.
	 */
	//Wait for enough room in FIFO to become available
	do {
		FifoFreeBytes = XLlFifo_TxVacancy(FifoInstancePtr);
	} while (FifoFreeBytes < ICMP_PKT_SIZE);

	//Write the frame data to FIFO
	XLlFifo_Write(FifoInstancePtr, (void*)BufAddr, ICMP_PKT_SIZE);

	//Initiate transmit
	XLlFifo_TxSetLen(FifoInstancePtr, ICMP_PKT_SIZE);

	//Wait for status of the transmitted packet
	switch (AxiEthernetPollForTxStatus()) {
	case XST_SUCCESS:/* Got a successful transmit status */
		break;

	case XST_NO_DATA:	/* Timed out */
		AxiEthernetUtilErrorTrap("Tx timeout");
		break;

	default:		/* Some other error */
		break;
	}
}

/*****************************************************************************/
/**
*
* This function will process the received packet. This function sends
* the echo request packet based on the ARP reply packet.
*
* @param	FifoInstancePtr is a pointer to the instance of the FIFO
*		component.
* @param	frameLength is the length of the received frame.
*
* @return	XST_SUCCESS is returned when an echo reply is received.
*		Otherwise, XST_FAILURE is returned.
*
* @note		This assumes MAC does not strip padding or CRC.
*
******************************************************************************/
int ProcessRecvFrame(XLlFifo *FifoInstancePtr, u32 frameLength)
{
	u16 *RxFramePtr;
	u16 *TempPtr;
	u16 CheckSum;
	int Index;
	int Match = 0;
	int DataWrong = 0;


	RxFramePtr = (u16 *)(UINTPTR)GetRxBufAddr();
	TempPtr = (u16 *)LocalMacAddr;

	//read the frame from the FIFO
	XLlFifo_Read(FifoInstancePtr, RxFramePtr, frameLength);

	/*
	 * Check Dest Mac address of the packet with the LocalMac address.
	 */
	if (Padding) {
		RxFramePtr += 4;
		Match = CompareData(RxFramePtr, TempPtr, 0, 0, MAC_ADDR_LEN);
	}
	if (Match == XST_SUCCESS) {
		/*
		 * Check ARP type.
		 */
		if (Xil_Ntohs(*(RxFramePtr + ETHER_PROTO_TYPE_LOC)) ==
				XAE_ETHER_PROTO_TYPE_ARP ) {

			/*
			 * Check ARP status.
			 */
			if (Xil_Ntohs(*(RxFramePtr + ARP_REQ_STATUS_LOC)) == ARP_REPLY) {

				/*
				 * Check destination IP address with
				 * packet's source IP address.
				 */
				TempPtr = (u16 *)DestIpAddress;
				Match = CompareData(RxFramePtr,
						TempPtr, ARP_REQ_SRC_IP_LOC,
						0, IP_ADDR_LEN);
				if (Match == XST_SUCCESS) {

					/*
					 * Copy src Mac address of the received
					 * packet.
					 */
					Index = MAC_ADDR_LEN;
					TempPtr = (u16 *)DestMacAddr;
					while (Index--) {
						*(TempPtr + Index) =
							*(RxFramePtr +
							(SRC_MAC_ADDR_LOC +
								Index));
					}

					/*
					 * Send Echo request packet.
					 */
					AxiEthernetUtilErrorTrap("Send an ICMP ping request packet");
					SendEchoReqFrame(FifoInstancePtr);
				}
				else {
					xdbg_printf(XDBG_DEBUG_ERROR, "ProcessRecvFrame: incoming IP does not match local IP\n");
				}
			}
		}

		/*
		 * Check for IP type.
		 */
		else if (Xil_Ntohs(*(RxFramePtr + ETHER_PROTO_TYPE_LOC)) ==
						XAE_ETHER_PROTO_TYPE_IP) {

			/*
			 * Calculate checksum.
			 */
			CheckSum = CheckSumCalculation(RxFramePtr,
							ICMP_DATA_START_LOC,
							ICMP_DATA_FIELD_LEN);

			/*
			 * Verify checksum, echo reply, identifier number and
			 * sequence number of the received packet.
			 */
			if ((CheckSum == CORRECT_CHECKSUM_VALUE) &&
			(Xil_Ntohs(*(RxFramePtr + ICMP_ECHO_FIELD_LOC)) == ECHO_REPLY) &&
			(Xil_Ntohs(*(RxFramePtr + ICMP_IDEN_FIELD_LOC)) == IDEN_NUM) &&
			(Xil_Ntohs(*(RxFramePtr + (ICMP_SEQ_NO_LOC))) == SeqNum)) {

				/*
				 * Verify data in the received packet with known
				 * data.
				 */
				TempPtr = IcmpData;
				Match = CompareData(RxFramePtr,
						TempPtr, ICMP_KNOWN_DATA_LOC,
							0, ICMP_KNOWN_DATA_LEN);
				if (Match == XST_FAILURE) {
					DataWrong = 1;
				}
			}
			if (DataWrong != 1) {
				AxiEthernetUtilErrorTrap("Echo Packet received");
				xdbg_printf(XDBG_DEBUG_GENERAL,
					"Packet No: {}\n", NUM_OF_PING_REQ_PKTS - NumOfPingReqPkts);
				xdbg_printf(XDBG_DEBUG_GENERAL,
					"Seq NO {} Echo Packet received\n", SeqNum);
				return XST_SUCCESS;
			}
			else {
				xdbg_printf(XDBG_DEBUG_ERROR,
					"ProcessRecvFrame: Packet No: {} wrong data\n",
					NUM_OF_PING_REQ_PKTS - NumOfPingReqPkts);
			}
		}
	}
	else {
		xdbg_printf(XDBG_DEBUG_ERROR, "ProcessRecvFrame: incoming MAC does not match local MAC\n");
		return XST_FAILURE;
	}

	return XST_SUCCESS;
}
/*****************************************************************************/
/**
*
* This function calculates the checksum and returns a 16 bit result.
*
* @param 	RxFramePtr is a 16 bit pointer for the data to which checksum
* 		is to be calculated.
* @param	StartLoc is the starting location of the data from which the
*		checksum has to be calculated.
* @param	Length is the number of halfwords(16 bits) to which checksum is
* 		to be calculated.
*
* @return	It returns a 16 bit checksum value.
*
* @note		This can also be used for calculating checksum. The ones
* 		complement of this return value will give the final checksum.
*
******************************************************************************/
static u16 CheckSumCalculation(u16 *RxFramePtr, int StartLoc, int Length)
{
	u32 Sum = 0;
	u16 CheckSum = 0;
	int Index;

	/*
	 * Add all the 16 bit data.
	 */
	Index = StartLoc;
	while (Index < (StartLoc + Length)) {
		Sum = Sum + Xil_Htons(*(RxFramePtr + Index));
		Index++;
	}

	/*
	 * Add upper 16 bits to lower 16 bits.
	 */
	CheckSum = Sum;
	Sum = Sum>>16;
	CheckSum = Sum + CheckSum;
	return CheckSum;
}
/*****************************************************************************/
/**
*
* This function checks the match for the specified number of half words.
*
* @param	LhsPtr is a LHS entity pointer.
* @param 	RhsPtr is a RHS entity pointer.
* @param	LhsLoc is a LHS entity location.
* @param 	RhsLoc is a RHS entity location.
* @param 	Count is the number of location which has to compared.
*
* @return	XST_SUCCESS is returned when both the entities are same,
*		otherwise XST_FAILURE is returned.
*
* @note		None.
*
******************************************************************************/
static int CompareData(u16 *LhsPtr, u16 *RhsPtr, int LhsLoc, int RhsLoc,
								int Count)
{
	int Result;
	while (Count--) {
		if (*(LhsPtr + LhsLoc + Count) == *(RhsPtr + RhsLoc + Count)) {
			Result = XST_SUCCESS;
		} else {
			Result = XST_FAILURE;
			break;
		}
	}
	return Result;
}
