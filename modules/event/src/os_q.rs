/*
*********************************************************************************************************
*                                                uC/OS-II
*                                          The Real-Time Kernel
*                                        MESSAGE QUEUE MANAGEMENT
*
*                              (c) Copyright 1992-2013, Micrium, Weston, FL
*                                           All Rights Reserved
*
*********************************************************************************************************
*/

// #[cfg(all(feature = "OS_Q_EN", feature = "OS_MAX_QS"))]


/*
*********************************************************************************************************
*                                      ACCEPT MESSAGE FROM QUEUE
*
* Description: This function checks the queue to see if a message is available.  Unlike OSQPend(),
*              OSQAccept() does not suspend the calling task if a message is not available.
*
* Arguments  : 
*
*
* Returns    : 
*
* Note(s)    : 
*
*********************************************************************************************************
*/

/// accept message
#[cfg(feature = "OS_Q_ACCEPT_EN")]
pub fn OSQAccept() {} 


/*
*********************************************************************************************************
*                                       CREATE A MESSAGE QUEUE
*
* Description: This function creates a message queue if free event control blocks are available.
*
* Arguments  : 
*
* Returns    : 
*
*********************************************************************************************************
*/

/// creates a message queue
pub fn OSQCreate() {}


/*
*********************************************************************************************************
*                                       DELETE A MESSAGE QUEUE
*
* Description: This function deletes a message queue and readies all tasks pending on the queue.
*
* Arguments  : 
*
* Returns    : 
*
* Note(s)    : 
*
*********************************************************************************************************
*/

#[cfg(feature = "OS_Q_DEL_EN")]
pub fn OSQDel() {}


/*
*********************************************************************************************************
*                                             FLUSH QUEUE
*
* Description : This function is used to flush the contents of the message queue.
*
* Arguments   : none
*
* Returns     : 
*
* WARNING     : 
*
*********************************************************************************************************
*/
#[cfg(feature = "OS_Q_FLUSH_EN")]
pub fn OSQFlush() {}


/*
*********************************************************************************************************
*                                    PEND ON A QUEUE FOR A MESSAGE
*
* Description: This function waits for a message to be sent to a queue
*
* Arguments  : 
*
* Returns    : 
*
* Note(s)    : 
*********************************************************************************************************
*/

pub fn OSQPend() {}


/*
*********************************************************************************************************
*                                       POST MESSAGE TO A QUEUE
*
* Description: This function sends a message to a queue
*
* Arguments  : 
*
* Returns    : 
*
* Note(s)    : 
*********************************************************************************************************
*/

#[cfg(feature = "OS_Q_POST_EN")]
pub fn OSQPost() {}


/*
*********************************************************************************************************
*                                POST MESSAGE TO THE FRONT OF A QUEUE
*
* Description: This function sends a message to a queue but unlike OSQPost(), the message is posted at
*              the front instead of the end of the queue.  Using OSQPostFront() allows you to send
*              'priority' messages.
*
* Arguments  : 
*
* Returns    : 
*
* Note(s)    : 
*********************************************************************************************************
*/

#[cfg(feature = "OS_Q_POST_FRONT_EN")]
pub fn OSQPostFront() {}


/// init the message queue
pub fn OS_QInit() {}
